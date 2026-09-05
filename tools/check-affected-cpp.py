#!/usr/bin/env python3
"""
check-affected-cpp.py

Fast, compiler-authoritative semantic checking for a CMake + Ninja project.

Normal mode:
    1. Read .build/compile_commands.json.
    2. Ask Ninja which object outputs are currently stale.
    3. Run ONLY those real translation units through the compiler in
       syntax/semantic-analysis-only mode.
    4. Run checks concurrently and keep each TU's diagnostics grouped.

This is intentionally not clang-tidy and not a replacement for the real build.
It is the fast "what did this type/ownership edit break?" loop between clangd
and `cmake --build`.

Designed for JTML's Pixi environment:
    pixi run python tools/check-affected-cpp.py

The script itself has no third-party Python dependencies.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

TU_SUFFIXES = {".c", ".cc", ".cpp", ".cxx", ".c++", ".cu"}

ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
CUDA_ARCH_RE = re.compile(r"(?:sm|compute)_([0-9]+[a-z]?)", re.IGNORECASE)

# Children are tracked so Bacon's kill_then_restart doesn't leave old compiler
# processes chewing CPU after the parent script is terminated.
_children: set[subprocess.Popen[str]] = set()
_children_lock = threading.Lock()
_cancelled = threading.Event()


@dataclass(frozen=True)
class CompileEntry:
    source: Path
    directory: Path
    output_abs: Path
    ninja_target: str
    argv: tuple[str, ...]


@dataclass(frozen=True)
class CheckResult:
    entry: CompileEntry
    returncode: int
    output: str
    elapsed: float
    command: tuple[str, ...]


def eprint(*args: object) -> None:
    print(*args, file=sys.stderr, flush=True)


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text)


def is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def resolve_from(directory: Path, value: str) -> Path:
    path = Path(value)
    if not path.is_absolute():
        path = directory / path
    return path.resolve(strict=False)


def command_argv(raw: dict) -> tuple[str, ...]:
    if "arguments" in raw:
        return tuple(str(x) for x in raw["arguments"])
    if "command" in raw:
        return tuple(shlex.split(raw["command"]))
    raise ValueError("compile_commands entry has neither 'arguments' nor 'command'")


def load_compile_database(build_dir: Path) -> list[CompileEntry]:
    db_path = build_dir / "compile_commands.json"
    if not db_path.is_file():
        raise RuntimeError(
            f"{db_path} does not exist.\n"
            "Configure CMake with CMAKE_EXPORT_COMPILE_COMMANDS=ON."
        )

    try:
        raw_entries = json.loads(db_path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"failed to read {db_path}: {exc}") from exc

    entries: list[CompileEntry] = []
    missing_output = 0

    for raw in raw_entries:
        directory = Path(raw["directory"]).resolve(strict=False)
        source = resolve_from(directory, raw["file"])

        if source.suffix.lower() not in TU_SUFFIXES:
            continue

        # Modern CMake writes this field. It gives us a lossless mapping from
        # compile_commands.json -> the exact Ninja object target.
        output = raw.get("output")
        if not output:
            missing_output += 1
            continue

        output_abs = resolve_from(directory, output)

        try:
            ninja_target = os.path.relpath(output_abs, build_dir)
        except ValueError:
            # Only realistically relevant on Windows with separate drives.
            ninja_target = str(output_abs)

        entries.append(
            CompileEntry(
                source=source,
                directory=directory,
                output_abs=output_abs,
                ninja_target=ninja_target,
                argv=command_argv(raw),
            )
        )

    if not entries:
        detail = (
            f" ({missing_output} TU entries lacked an 'output' field)"
            if missing_output
            else ""
        )
        raise RuntimeError(f"no usable translation units in {db_path}{detail}")

    if missing_output:
        eprint(
            f"warning: skipped {missing_output} compile_commands entries without "
            "an 'output' field"
        )

    return entries


def run_capture(
    argv: Sequence[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(argv),
        cwd=str(cwd) if cwd else None,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        check=False,
    )


def ninja_deps(
    build_dir: Path,
    entries: Sequence[CompileEntry],
    ninja: str,
) -> dict[str, list[Path] | None]:
    """
    Read Ninja's compiler-discovered dependency log for all object targets.

    This deliberately does NOT use `ninja -n`.

    CMake projects using file(GLOB ... CONFIGURE_DEPENDS) add an always-run
    VerifyGlobs step to the manifest regeneration edge. A dry-run cannot
    execute/restat that step, so `ninja -n` can pessimistically report that
    CMake would regenerate even when an ordinary build would simply verify
    the glob and continue. The deps tool reads .ninja_deps directly and does
    not run build-system maintenance.

    Return:
        {ninja_target: [dependency paths]}
        {ninja_target: None} when no deps entry exists yet.
    """
    targets = [entry.ninja_target for entry in entries]
    cmd = [ninja, "-C", str(build_dir), "-t", "deps", *targets]

    try:
        proc = run_capture(cmd)
    except OSError as exc:
        if exc.errno != getattr(os, "E2BIG", 7):
            raise

        # Very large projects may exceed ARG_MAX. Chunk without changing
        # semantics; `-t deps` accepts multiple targets.
        outputs: list[str] = []
        for start in range(0, len(targets), 256):
            chunk = run_capture(
                [
                    ninja,
                    "-C",
                    str(build_dir),
                    "-t",
                    "deps",
                    *targets[start : start + 256],
                ]
            )
            if chunk.returncode != 0:
                raise RuntimeError(
                    "Ninja dependency query failed:\n" + chunk.stdout.rstrip()
                )
            outputs.append(chunk.stdout)
        output = "".join(outputs)
    else:
        if proc.returncode != 0:
            raise RuntimeError(
                "Ninja dependency query failed:\n" + proc.stdout.rstrip()
            )
        output = proc.stdout

    # ToolDeps prints blocks like:
    #
    #   path/to/foo.o: #deps 17, deps mtime ... (VALID)
    #       /absolute/source.cpp
    #       /absolute/header.hpp
    #
    # or:
    #
    #   path/to/foo.o: deps not found
    #
    result: dict[str, list[Path] | None] = {}
    current_target: str | None = None

    header_re = re.compile(
        r"^(.*): (?:(?:#deps \d+, deps mtime .*\((?:VALID|STALE)\))|deps not found)$"
    )

    for raw_line in strip_ansi(output).splitlines():
        line = raw_line.rstrip()

        if not line:
            current_target = None
            continue

        match = header_re.match(line)
        if match:
            current_target = match.group(1)
            result[current_target] = None if line.endswith("deps not found") else []
            continue

        if current_target is not None and raw_line[:1].isspace():
            deps = result[current_target]
            if deps is None:
                continue

            dep_text = line.strip()
            if not dep_text:
                continue

            dep = Path(dep_text)
            if not dep.is_absolute():
                dep = build_dir / dep
            deps.append(dep.resolve(strict=False))

    # If Ninja canonicalized a target spelling differently from the path we
    # passed, map by resolved output path as a fallback.
    by_abs_output = {str(entry.output_abs): entry.ninja_target for entry in entries}

    normalized: dict[str, list[Path] | None] = {}
    for target, deps in result.items():
        target_path = Path(target)
        if not target_path.is_absolute():
            target_path = build_dir / target_path
        target_abs = str(target_path.resolve(strict=False))
        normalized[by_abs_output.get(target_abs, target)] = deps

    return normalized


def find_stale_entries(
    build_dir: Path,
    entries: Sequence[CompileEntry],
    ninja: str,
) -> list[CompileEntry]:
    """
    Select translation units whose existing object is older than their source
    or any compiler-discovered dependency.

    This is intentionally a semantic-check invalidation test, not a complete
    reimplementation of Ninja's build-edge dirtiness rules. In particular,
    command-line/CMake graph changes remain the responsibility of the real
    build. For edit/refactor feedback, source/header dependency invalidation is
    the signal we care about.
    """
    dep_map = ninja_deps(build_dir, entries, ninja=ninja)
    stale: list[CompileEntry] = []

    for entry in entries:
        try:
            object_mtime = entry.output_abs.stat().st_mtime_ns
        except FileNotFoundError:
            stale.append(entry)
            continue
        except OSError as exc:
            raise RuntimeError(
                f"failed to stat object {entry.output_abs}: {exc}"
            ) from exc

        deps = dep_map.get(entry.ninja_target)

        # No deps record normally means the TU has never successfully produced
        # dependency information. Be conservative and check it.
        if deps is None:
            stale.append(entry)
            continue

        # The source is always semantically relevant even if a compiler's
        # depfile happened not to record it.
        candidates = [entry.source, *deps]

        is_stale = False
        for dep in candidates:
            try:
                dep_mtime = dep.stat().st_mtime_ns
            except FileNotFoundError:
                # A dependency disappeared. The semantic checker should get a
                # chance to report the resulting include/error rather than
                # silently treating the TU as clean.
                is_stale = True
                break
            except OSError as exc:
                raise RuntimeError(f"failed to stat dependency {dep}: {exc}") from exc

            if dep_mtime > object_mtime:
                is_stale = True
                break

        if is_stale:
            stale.append(entry)

    return stale


def compiler_index(argv: Sequence[str]) -> int:
    """
    Skip common compiler launchers. We intentionally bypass them for syntax
    checks: ccache/sccache add no value when no object file is produced.
    """
    launchers = {"ccache", "sccache"}

    for idx, arg in enumerate(argv[:3]):
        if Path(arg).name not in launchers:
            return idx

    return 0


def compiler_name(argv: Sequence[str]) -> str:
    idx = compiler_index(argv)
    return Path(argv[idx]).name.lower()


def remove_codegen_and_dep_flags(args: Sequence[str]) -> list[str]:
    """
    Preserve semantic flags (-D/-I/-std/-O/etc.) while removing only outputs,
    dependency-file emission, and the compile action that writes an object.
    """
    result: list[str] = []
    i = 0

    takes_value_and_drop = {
        "-o",
        "-MF",
        "-MT",
        "-MQ",
        "--dependency-output",
        "--serialize-diagnostics",
    }
    drop_exact = {
        "-c",
        "--compile",
        "-MD",
        "-MMD",
        "-MP",
        "-MG",
    }

    while i < len(args):
        arg = args[i]

        if arg in takes_value_and_drop:
            i += 2
            continue

        if arg in drop_exact:
            i += 1
            continue

        if arg.startswith("-o") and arg != "-ObjC" and len(arg) > 2:
            i += 1
            continue

        if any(arg.startswith(prefix) for prefix in ("-MF", "-MT", "-MQ")):
            i += 1
            continue

        result.append(arg)
        i += 1

    return result


def unwrap_host_flags(value: str) -> list[str]:
    # NVCC commonly encodes -Xcompiler=-fPIC,-pthread.
    return [part for part in value.split(",") if part]


def nvcc_to_clang(
    entry: CompileEntry,
    argv: Sequence[str],
    clangxx: str,
    cuda_host_only: bool,
) -> list[str]:
    """
    Translate the semantic subset of a normal CMake NVCC compile command to a
    Clang CUDA syntax-only invocation.

    The goal is type/ownership/include/template checking, not reproducing PTXAS
    or linker/codegen behavior.
    """
    idx = compiler_index(argv)
    nvcc = Path(argv[idx])
    nvcc_resolved = Path(shutil.which(str(nvcc)) or nvcc).resolve(strict=False)

    # Typical layout: <cuda-root>/bin/nvcc
    cuda_root = nvcc_resolved.parent.parent

    body = list(argv[idx + 1 :])
    kept: list[str] = []
    arch: str | None = None
    relaxed_constexpr = False
    extended_lambda = False
    relocatable_device_code = False

    i = 0
    while i < len(body):
        arg = body[i]

        arch_match = CUDA_ARCH_RE.search(arg)
        if arch_match and arch is None:
            arch = arch_match.group(1)

        # Action/output/dependency flags.
        if arg in {
            "-c",
            "--compile",
            "-dc",
            "--device-c",
            "-dw",
            "--device-w",
            "-MD",
            "-MMD",
            "-MP",
            "-MG",
        }:
            i += 1
            continue

        if arg in {"-o", "-MF", "-MT", "-MQ"}:
            i += 2
            continue

        if any(arg.startswith(p) for p in ("-o", "-MF", "-MT", "-MQ")):
            i += 1
            continue

        # NVCC control flags that don't affect parsing/sema.
        if arg in {
            "--forward-unknown-to-host-compiler",
            "-forward-unknown-to-host-compiler",
            "-lineinfo",
            "--generate-line-info",
            "--use_fast_math",
        }:
            i += 1
            continue

        if arg in {"-ccbin", "--compiler-bindir", "--threads"}:
            i += 2
            continue

        if arg.startswith(("-ccbin=", "--compiler-bindir=", "--threads=")):
            i += 1
            continue

        if arg in {"-gencode", "--generate-code"}:
            if i + 1 < len(body):
                match = CUDA_ARCH_RE.search(body[i + 1])
                if match and arch is None:
                    arch = match.group(1)
            i += 2
            continue

        if arg.startswith(("-gencode=", "--generate-code=")):
            i += 1
            continue

        if arg in {"--expt-relaxed-constexpr"}:
            relaxed_constexpr = True
            i += 1
            continue

        if arg in {"--extended-lambda", "--expt-extended-lambda"}:
            extended_lambda = True
            i += 1
            continue

        if arg in {"-rdc=true", "--relocatable-device-code=true"}:
            relocatable_device_code = True
            i += 1
            continue

        if arg in {"-rdc=false", "--relocatable-device-code=false"}:
            i += 1
            continue

        if arg in {"-Xcompiler", "--compiler-options"}:
            if i + 1 < len(body):
                kept.extend(unwrap_host_flags(body[i + 1]))
            i += 2
            continue

        if arg.startswith(("-Xcompiler=", "--compiler-options=")):
            kept.extend(unwrap_host_flags(arg.split("=", 1)[1]))
            i += 1
            continue

        # Preserve include/define/language/host-semantic options.
        if arg in {"-I", "-isystem", "-D", "-U", "-include"}:
            if i + 1 < len(body):
                kept.extend([arg, body[i + 1]])
            i += 2
            continue

        if arg.startswith(("-I", "-D", "-U", "-std=", "--std=")):
            if arg.startswith("--std="):
                arg = "-std=" + arg.split("=", 1)[1]
            kept.append(arg)
            i += 1
            continue

        if arg.startswith("-isystem"):
            kept.append(arg)
            i += 1
            continue

        # Keep common host semantic flags. -O is kept intentionally because it
        # changes preprocessor macros such as __OPTIMIZE__, even in syntax-only.
        if arg.startswith(("-O", "-g", "-W", "-f", "-m")) or arg == "-pthread":
            kept.append(arg)
            i += 1
            continue

        # Ignore the source spelling from compile_commands and append a known
        # absolute path once at the end.
        if (
            not arg.startswith("-")
            and resolve_from(entry.directory, arg) == entry.source
        ):
            i += 1
            continue

        # Remaining NVCC-specific switches are deliberately ignored. `--verbose`
        # shows the transformed command when investigating an edge case.
        i += 1

    cmd = [
        clangxx,
        "-x",
        "cuda",
        f"--cuda-path={cuda_root}",
    ]

    if arch:
        cmd.append(f"--cuda-gpu-arch=sm_{arch}")

    if cuda_host_only:
        cmd.append("--cuda-host-only")

    if relaxed_constexpr:
        cmd.append("-D__CUDACC_RELAXED_CONSTEXPR__=1")

    if extended_lambda:
        cmd.append("-D__CUDACC_EXTENDED_LAMBDA__=1")

    if relocatable_device_code:
        cmd.append("-fgpu-rdc")

    cmd.extend(kept)
    cmd.extend(
        [
            "-fsyntax-only",
            "-ferror-limit=0",
            "-fdiagnostics-color=always",
            str(entry.source),
        ]
    )
    return cmd


def syntax_command(
    entry: CompileEntry,
    clangxx: str,
    cuda_host_only: bool,
) -> list[str]:
    argv = list(entry.argv)
    idx = compiler_index(argv)
    actual_compiler = argv[idx]
    name = Path(actual_compiler).name.lower()

    is_cuda = entry.source.suffix.lower() == ".cu"

    if is_cuda and "nvcc" in name:
        return nvcc_to_clang(
            entry,
            argv,
            clangxx=clangxx,
            cuda_host_only=cuda_host_only,
        )

    # Bypass ccache/sccache but otherwise preserve the actual configured compiler.
    body = remove_codegen_and_dep_flags(argv[idx + 1 :])
    cmd = [actual_compiler, *body]

    if "-fsyntax-only" not in cmd:
        cmd.append("-fsyntax-only")

    if "clang" in name:
        cmd.extend(["-ferror-limit=0", "-fdiagnostics-color=always"])
        if is_cuda and cuda_host_only:
            cmd.append("--cuda-host-only")
    elif name in {"gcc", "g++", "c++"} or "gcc" in name or "g++" in name:
        cmd.extend(["-fmax-errors=0", "-fdiagnostics-color=always"])

    return cmd


def register_child(proc: subprocess.Popen[str]) -> None:
    with _children_lock:
        _children.add(proc)


def unregister_child(proc: subprocess.Popen[str]) -> None:
    with _children_lock:
        _children.discard(proc)


def terminate_children() -> None:
    _cancelled.set()
    with _children_lock:
        children = list(_children)

    for proc in children:
        if proc.poll() is None:
            try:
                proc.terminate()
            except ProcessLookupError:
                pass


def install_signal_handlers() -> None:
    def handler(signum: int, _frame: object) -> None:
        terminate_children()
        raise KeyboardInterrupt

    signal.signal(signal.SIGINT, handler)
    signal.signal(signal.SIGTERM, handler)


def check_one(
    entry: CompileEntry,
    clangxx: str,
    cuda_host_only: bool,
) -> CheckResult:
    if _cancelled.is_set():
        return CheckResult(entry, 130, "", 0.0, ())

    cmd = syntax_command(
        entry,
        clangxx=clangxx,
        cuda_host_only=cuda_host_only,
    )

    start = time.monotonic()
    proc = subprocess.Popen(
        cmd,
        cwd=str(entry.directory),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
    )
    register_child(proc)

    try:
        output, _ = proc.communicate()
    finally:
        unregister_child(proc)

    return CheckResult(
        entry=entry,
        returncode=proc.returncode,
        output=output or "",
        elapsed=time.monotonic() - start,
        command=tuple(cmd),
    )


def pretty_source(source: Path, project_root: Path) -> str:
    if is_relative_to(source, project_root):
        return str(source.relative_to(project_root))
    return str(source)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Syntax/sema-check only the C/C++/CUDA translation units Ninja says "
            "are stale."
        )
    )
    parser.add_argument(
        "-B",
        "--build-dir",
        default=".build",
        help="CMake/Ninja build directory (default: .build)",
    )
    parser.add_argument(
        "-j",
        "--jobs",
        type=int,
        default=int(os.environ.get("JTML_CPP_CHECK_JOBS", "0")),
        help=(
            "parallel compiler processes; 0 = os.cpu_count() "
            "(default: env JTML_CPP_CHECK_JOBS or 0)"
        ),
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="check every TU in compile_commands.json instead of only stale TUs",
    )
    parser.add_argument(
        "--cuda-host-only",
        action="store_true",
        help=(
            "for CUDA TUs, ask Clang to check the host compilation only "
            "(faster, but less exhaustive)"
        ),
    )
    parser.add_argument(
        "--clangxx",
        default=os.environ.get("CLANGXX", "clang++"),
        help="clang++ used to translate NVCC CUDA commands (default: clang++)",
    )
    parser.add_argument(
        "--ninja",
        default=os.environ.get("NINJA", "ninja"),
        help="Ninja executable (default: ninja)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="print selected TUs, transformed commands, and timings",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    install_signal_handlers()

    project_root = Path.cwd().resolve(strict=False)
    build_dir = Path(args.build_dir)
    if not build_dir.is_absolute():
        build_dir = project_root / build_dir
    build_dir = build_dir.resolve(strict=False)

    ninja = shutil.which(args.ninja)
    if not ninja:
        eprint(
            f"error: '{args.ninja}' not found in PATH. "
            "Run this through the JTML Pixi environment."
        )
        return 2

    clangxx = shutil.which(args.clangxx)
    if not clangxx:
        eprint(
            f"error: '{args.clangxx}' not found in PATH. "
            "Run this through the JTML Pixi environment."
        )
        return 2

    try:
        entries = load_compile_database(build_dir)
        selected = (
            entries if args.all else find_stale_entries(build_dir, entries, ninja=ninja)
        )
    except RuntimeError as exc:
        eprint(f"error: {exc}")
        return 2

    if not selected:
        print("cpp-check: no stale translation units")
        return 0

    jobs = args.jobs if args.jobs > 0 else (os.cpu_count() or 1)
    jobs = max(1, min(jobs, len(selected)))

    cuda_count = sum(e.source.suffix.lower() == ".cu" for e in selected)
    print(
        f"cpp-check: {len(selected)} stale TU"
        f"{'' if len(selected) == 1 else 's'} "
        f"({cuda_count} CUDA), {jobs} workers",
        flush=True,
    )

    if args.verbose:
        for entry in selected:
            print(f"  {pretty_source(entry.source, project_root)}")
        print(flush=True)

    failures = 0
    diagnostics = 0
    started = time.monotonic()

    try:
        with concurrent.futures.ThreadPoolExecutor(
            max_workers=jobs,
            thread_name_prefix="cpp-check",
        ) as pool:
            future_map = {
                pool.submit(
                    check_one,
                    entry,
                    clangxx,
                    args.cuda_host_only,
                ): entry
                for entry in selected
            }

            completed = 0
            for future in concurrent.futures.as_completed(future_map):
                completed += 1
                entry = future_map[future]

                try:
                    result = future.result()
                except Exception as exc:
                    failures += 1
                    eprint(
                        f"{pretty_source(entry.source, project_root)}: "
                        f"checker failed: {exc}"
                    )
                    continue

                if result.output.strip():
                    diagnostics += 1
                    # Whole-TU output is emitted atomically, so diagnostics from
                    # parallel compiler processes never interleave.
                    sys.stdout.write(result.output)
                    if not result.output.endswith("\n"):
                        sys.stdout.write("\n")
                    sys.stdout.flush()

                if result.returncode != 0:
                    failures += 1

                if args.verbose:
                    status = "FAIL" if result.returncode else "ok"
                    print(
                        f"[{completed}/{len(selected)}] {status:4} "
                        f"{result.elapsed:6.2f}s  "
                        f"{pretty_source(result.entry.source, project_root)}"
                    )
                    print("    " + shlex.join(result.command))

    except KeyboardInterrupt:
        terminate_children()
        eprint("cpp-check: cancelled")
        return 130

    elapsed = time.monotonic() - started
    print(
        f"cpp-check: {len(selected)} TU"
        f"{'' if len(selected) == 1 else 's'} checked in {elapsed:.2f}s; "
        f"{failures} failed; {diagnostics} emitted diagnostics",
        flush=True,
    )

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
