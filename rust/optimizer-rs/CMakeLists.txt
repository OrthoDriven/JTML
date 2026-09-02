
corrosion_add_cxxbridge(optimizer-rs_bridge
  CRATE optimizer_rs
  REGEN_TARGET optimizer_rs_bridge_regen
  FILES bridge.rs
)

option(USE_RUST_DIRECT "Use Rust-based direct optimizer" 1)

target_link_libraries(
  optimizer-rs_bridge
  PUBLIC
    optimizer_rs
)
target_link_libraries(jtml_coordinator PUBLIC optimizer-rs_bridge)
target_link_libraries(jtml_domain PUBLIC optimizer-rs_bridge)

target_include_directories(
  optimizer-rs_bridge
  PUBLIC
    ${PROJECT_SOURCE_DIR}/include
)

target_compile_definitions(jtml_coordinator PRIVATE
  USE_RUST_DIRECT=$<BOOL:${USE_RUST_DIRECT}>)
target_compile_definitions(jtml_domain PRIVATE
  USE_RUST_DIRECT=$<BOOL:${USE_RUST_DIRECT}>)
