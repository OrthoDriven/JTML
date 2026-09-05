# Agent Brief: Reimplementing the JTML X-ray/3D Overlay Renderer in JavaScript (vtk-js)

## 0. What you are building (the mission)

You are building a **calibrated 2D/3D overlay viewer** for single-plane X-ray
fluoroscopy (knee-implant registration). The application must:

1. Load a **calibration file** describing one X-ray camera (a pinhole model).
2. Load **multiple X-ray images** (frames), each with the same pixel dimensions,
   and display one at a time as the viewport background.
3. Load **multiple 3D models** (STL CAD models of implants/bones) and overlay them
   on the X-ray image, positioned by a **6-DOF pose per (frame, model) pair**, using
   a perspective camera whose projection **exactly reproduces the calibrated pinhole
   model** of the X-ray source/detector.

The critical property: **a model pose rendered by this viewer must project onto the
background image exactly the way the calibration says**. This viewer is the visual
ground truth for a pose-optimization system, so camera math precision is not
optional. Every sign convention below is load-bearing.

This document describes WHAT to build and the exact mathematics. It deliberately
does not prescribe library calls — the C++ original uses VTK; you will use vtk-js
(or equivalent WebGL). Where behavior is pinned by the original implementation, the
formula is given verbatim and marked **[PINNED]**.

---

## 1. The big picture

The original renders **two stacked layers in one viewport**, each with its own camera:

```
┌──────────────────────────────────────────────┐
│  Layer 1 (front): 3D scene        PERSPECTIVE │   ← STL models, poses
│  Camera: pinhole replica of the X-ray camera  │
├──────────────────────────────────────────────┤
│  Layer 0 (back): background image ORTHOGRAPHIC│   ← the X-ray frame
│  Camera: straight-on, image fills viewport     │
└──────────────────────────────────────────────┘
```

- **Layer 0** draws the current X-ray image so that it *exactly fills the viewport*,
  1 image pixel = 1 screen unit, centered on the optical axis. It uses an
  **orthographic (parallel) projection**, so its camera depth parameters barely
  matter — only framing.
- **Layer 1** is a normal perspective 3D scene. Its camera is constructed so that
  its perspective projection is *mathematically identical* to the calibrated
  pinhole projection `sX = (X/Z)·fx + cx`, `sY = (Y/Z)·fy + cy` (in image pixel
  coordinates, y-up, origin bottom-left). STL models are placed with their stored
  6-DOF poses; whatever lands on layer 1 appears overlaid on the X-ray.

The world coordinate system (for both layers) is defined as:

- **Origin**: the X-ray source.
- **+X**: to the right on the image. **+Y**: *up* on the image. **+Z**: *toward the
  viewer* (out of the image plane, toward the camera/source). Right-handed.
- **Units: PIXELS.** This is a key quirk. The calibration file is in millimeters;
  all millimeter quantities are converted to pixels by dividing by the pixel pitch
  at load time. After conversion, the entire 3D scene lives in pixel units: a model
  at pose z = −4000 is 4000 px in front of the source.
- Image pixel coordinates on the background are **y-up, origin bottom-left** (see
  §4 — images are flipped vertically at load to achieve this).

---

## 2. Calibration file loading

The calibration file is a whitespace/comma/tab/newline-delimited list of numbers
with a leading **type code** as the first token. Split on the regex
`[\r\n]|,|\t| ` and drop empty tokens. The only supported format:

### 2.1 "UF / JointTrack" monoplane format — code `JT_INTCALIB` or `JTA_INTCALIB`

| Token index | Meaning |
|---|---|
| 0 | type code |
| 1 | `principal_distance` — source-to-detector distance, **mm** |
| 2 | `principal_x` — principal point offset, **mm** |
| 3 | `principal_y` — principal point offset, **mm** |
| 4 | `pixel_pitch` — detector pixel size, **mm/pixel** |

**Error check:** if token 4 is 0 → reject ("pixel size is zero").

**[PINNED] Sign flip:** `principal_x` and `principal_y` are **negated on load**
(token 2 and token 3), for historical JointTrack compatibility. Store the negated
values as the calibration's principal point. Every downstream computation uses the
negated values. Do not "fix" this.

**Derived camera parameters (all in pixels):**

```
fx = fy = principal_distance / pixel_pitch
cx = principal_x / pixel_pitch        (after negation)
cy = principal_y / pixel_pitch        (after negation)
```

Calibration type marker: `"UF"`.

**If the first token is neither `JT_INTCALIB` nor `JTA_INTCALIB` → reject
("invalid configuration file").**

### 2.2 Session rules

- Calibration must be loaded **before** images or models (the other loaders reject
  with "load calibration first").
- Calibration is **one-use per session**: after a successful load, the loader is
  disabled. Replacing the dataset later keeps the existing calibration.

---

## 3. The two camera models (the heart of the math)

### 3.1 The ideal pinhole model (what projection must satisfy)

For a 3D point `P = (X, Y, Z)` in the camera's world frame (Z < 0 = in front of the
source, since +Z points back toward the source):

```
sX = (X / Z) · fx + cx
sY = (Y / Z) · fy + cy
```

where `(sX, sY)` are **image pixel coordinates, y-up, origin bottom-left**, and
`fx, fy, cx, cy` are the derived pixel-unit parameters from §2. This is the exact
projection used by the original system's GPU renderer, so the 3D overlay must
match it exactly.

Note the consequence: because Z is negative in front of the camera, the division
by Z flips signs — a point at X = +100, Z = −1000 lands at sX = cx − 0.1·fx, i.e.
*left* of center. This is consistent and correct for this convention; keep it.

### 3.2 Background-layer camera (orthographic)

Requirements:

- Camera at world origin `(0,0,0)`, looking straight down **−Z** (focal point
  `(0,0,−1)` is sufficient to define the direction).
- **Orthographic projection** whose vertical half-extent is `height/2` world units
  (in VTK terms: parallel projection on, parallel scale = `0.5 · image_height_px`).
  Result: 1 world unit = 1 image pixel; the viewport shows exactly the image
  extent.
- Clipping range roughly `(0.1, 2 · fy)` — generous; the image sits at
  `z = −fy · pixel_pitch` (see below), well inside it.
- The image actor/plane is positioned so the image is **centered on the optical
  axis**: its lower-left corner at

```
position = (−0.5 · image_width, −0.5 · image_height, z_img)
z_img = −fy · pixel_pitch            (= −principal_distance, a mm value reused as scene depth)
```

Since the projection is orthographic, `z_img` only needs to be inside the clip
range; the value above is the original's choice **[PINNED]**.

### 3.3 Scene-layer camera (perspective = calibrated pinhole replica)

This is the delicate part. The camera must be built so that its projection equals
§3.1 for any point. Four pieces:

**(a) Position and direction.** Camera at `(0,0,0)`, focal point `(0,0,−1)`
**[PINNED]** — the camera looks straight down −Z, matching the pinhole convention
above.

**(b) Vertical view angle** so that the perspective frustum has focal length `fy`
in pixel units. With the camera 1 world unit from its focal point, a vertical
half-angle θ satisfies `tan(θ) = (h/2) / fy`, hence **[PINNED]**:

```
viewAngle_deg = (180/π) · 2 · atan2(h, 2·fy)          // h = image height in px
```

This yields a frustum whose focal length is exactly `fy` pixels.

**(c) Principal point (window center).** The projection center must sit at
`(cx, cy)` relative to the image center. Implement via a "window center" /
frustum-offset mechanism (VTK: `camera->SetWindowCenter(x, y)`; in a raw
projection matrix this is the 3rd-column x/y entries of the projection, i.e. an
offset in normalized half-extent units). **[PINNED] formulas** (cx,cy are
principal points in px; w,h = image size):

```
cx' = w/2 − cx ;  cy' = h/2 + cy
windowCenterX = −(2·cx' − w) / w      // simplifies to +2·cx/w
windowCenterY =  (2·cy' − h) / h      // simplifies to +2·cy/h
```

Net effect: the projection center is offset by exactly `(cx, cy)` pixels from the
image center — i.e. the projection becomes `(X/Z)·fx + cx, (Y/Z)·fy + cy`, as
required.

**On window resize**, keep the mapping consistent: if the window is wider than
tall, scale the x component by `window_height/window_width`, else scale the y
component by `window_width/window_height` **[PINNED]**:

```
rww > rwh ?  SetWindowCenter(wcx · (rwh/rww), wcy)
           :  SetWindowCenter(wcx, wcy · (rww/rwh))
```

**(d) Clipping range and aspect.** **[PINNED]**

```
clipping range = (0.1 · fx, 1.75 · fx)
```

Set a **user/world transform on the camera that scales X by `fx/fy`** (a diagonal
matrix with `fx/fy` in the x-scale slot, applied to the projection). For the
usual square-pixel case `fx == fy`, this is identity; it exists to support
non-square pixels.

**Sanity check you can run after building the camera:** a point at
`(cx, cy, −fy)` must project to the exact center of the viewport, and a point at
`(0, 0, −fy)` must project to the point `w/2 − cx`, `h/2 − cy` in the viewport
measured y-up from bottom-left. If either fails, a sign is wrong.

### 3.4 Model default pose

Every newly loaded model receives, for every frame (and for a "no image yet"
state), the pose:

```
x = 0, y = 0, z = −0.25 · principal_distance / pixel_pitch   (= −0.25·fy px)
rotations all 0
```

I.e. the model pops in at **¼ of the focal length in front of the source**, centered
on the optical axis, unrotated.

---

## 4. Image (frame) loading

For each selected image file:

1. Decode to an **8-bit single-channel (grayscale)** image.
2. **Flip vertically** (row order reversal). This converts from the decoder's
   top-left origin to the y-up bottom-left convention used by the renderer and the
   calibration math. Do this once, at load, on the stored image.
3. Compute and cache the derived variants (each is a grayscale image of the same
   size):
   - **Inverted**: `out = 255 − in`.
   - **Edges**: Canny edge detection with three parameters — aperture size
     (Sobel kernel size, odd), low threshold, high threshold. Each frame stores
     its own triple; the UI edits them per frame.
   - **Dilated edges**: morphological dilation of the edge image with a 3×3
     rectangular kernel, iterated `dilation` times (`dilation` count per frame).
   - (The original also computes a distance transform of the inverted edges for
     the cost function; a viewer does not need it, but the edge/dilation images
     should exist because they are display modes.)
4. **Size gate:** every frame must have identical width and height, including
   previously loaded frames. On mismatch, **abort mid-list but keep all frames
   loaded so far** (partial load is the pinned behavior).
5. Each loaded frame increments the frame count; for each frame, every already
   loaded model gets a fresh pose slot (initialized to §3.4 default).

**Display:** selecting a frame displays one of its four variants
(original/inverted/edges/dilated) as the layer-0 background via the §3.2
mechanism. Changing the variant radio re-imports the current frame's selected
variant. Switching frames re-imports and **re-applies every selected model's pose
for that frame** (poses are per-frame; see §6).

---

## 5. Model (STL) loading

For each selected STL file:

1. Parse the STL (binary or ASCII) into triangle soup: vertex positions + per-face
   normals. If parsing fails, still create the model entry but flag it
   "possibly corrupted" and warn (pinned behavior: warn and continue).
2. Create one renderable actor: geometry → mapper → actor in the **scene layer**.
3. All models start **invisible and non-pickable**.
4. Assign each model a default pose in the pose store (§3.4) for every existing
   frame, and register it so future frames get slots too.
5. Models may be loaded **before or after** images; ordering must not matter.

**Selection & style:**

- The model list supports multi-selection. Selected models become visible; the
  first (primary) selected model is tinted **orange RGB(255, 77, 0)**, all others
  **blue RGB(0, 72, 204)** (0–255 scale → normalize).
- Display modes per model (radio): *original* (Lambertian: ambient 0, diffuse 1,
  opacity 1), *solid* (unlit: ambient 1, diffuse 0, opacity 1), *transparent*
  (unlit, opacity 0.2), *wireframe* (edges only, opacity 1).
- If a frame is selected, apply that frame's stored pose to every selected model
  on display.

---

## 6. Poses: representation and application — the rotation convention is critical

### 6.1 Storage

A **pose** is 6 numbers: translation `(x, y, z)` in **pixels** and rotation
`(xa, ya, za)` in **degrees**. Poses live in a `frames × models` matrix (plus a
pose list for the "no frame selected" state).

### 6.2 Applying a pose to a model actor

The model's local vertices are transformed to world by:

```
p_world = R · p_model + t          (column vectors)
t = (x, y, z)
R = Rz(za) · Rx(xa) · Ry(ya)       (angles degrees → radians; right-handed, active rotation)
```

This **Z·X·Y composition is pinned** — it is what the original's GPU cost
function uses (`R·v` with R = Rz·Rx·Ry), and what VTK's actor orientation
produces. The elementary matrices (right-handed, counterclockwise when looking
down the axis toward the origin):

```
Rx(θ) = [1    0      0   ; 0  cosθ  −sinθ ; 0  sinθ  cosθ]
Ry(θ) = [cosθ  0   sinθ  ; 0    1     0   ; −sinθ 0  cosθ]
Rz(θ) = [cosθ −sinθ  0   ; sinθ  cosθ  0   ; 0    0    1  ]
```

Explicit expanded form of `R = Rz·Rx·Ry` (for validating your implementation):

```
R[0][0] = cz·cy − sz·sx·sy      R[0][1] = −sz·cx      R[0][2] = cz·sy + sz·cy·sx
R[1][0] = sz·cy + cz·sx·sy      R[1][1] =  cz·cx      R[1][2] = sz·sy − cz·cy·sx
R[2][0] = −cx·sy                R[2][1] =  sx         R[2][2] = cx·cy
```

(ck = cos(angle_k), sk = sin(angle_k).) **Recommendation:** do not rely on the
library's Euler-angle setter; compose the 4×4 matrix yourself from R and t and set
it directly, so the order is unambiguous. If you do use an orientation API, verify
it composes in the order RotateY, then RotateX, then RotateZ (post-multiplied),
which yields R = Rz·Rx·Ry.

### 6.3 Round-tripping

Dragging a model in the viewport reads back the actor's position/orientation and
writes it to the pose store for (current frame, model). Selecting a different
frame saves the on-screen poses of the outgoing frame first, then applies the
incoming frame's stored poses.

---

## 7. Application state and load-order semantics

Maintain:

- `calibration` (optional-once): type marker + pixel-unit intrinsics
  (`fx, fy, cx, cy`, pixel pitch, principal distance).
- `frames: Frame[]`. A `Frame` = file path + 4 image variants + Canny params
  (aperture, low, high) + dilation count.
- `models: Model[]` (path, display name, geometry, validity flag).
- `poses: Frame[] × Model[] → Pose` (plus a pose list for "no frame selected").
- UI state: current frame index, selected models, display modes for image and
  models.

Ordering rules (pinned):

1. Calibration first; one-shot.
2. Images: all same size; partial loads persist on mismatch.
3. Models: any time after calibration; each gets default poses for all frames.
4. Selecting a frame ⇒ display its image variant + apply stored poses of selected
   models for that frame.
5. Selecting models ⇒ show them (first-selected orange, rest blue), apply current
   frame's poses.

---

## 8. Interaction (viewer modes)

- **Camera interaction mode:** orbit/pan/zoom of the *scene* camera (a trackball
  camera style). The background layer stays fixed — it is the patient's X-ray and
  must never move. (Panning/orbiting the scene camera is for inspecting models in
  3D; a "reset view" action restores the §3.3 camera.)
- **Model interaction mode:** ray-pick a model actor and drag to translate /
  rotate it; on release (and on any selection change), the pose is written back to
  the store (§6.3). Multi-model: only picked model moves.

---

## 9. Pitfall checklist (read before implementing; each of these was a real bug)

1. **Units:** the calibration file is mm; the 3D scene is pixels. Divide by pixel
   pitch exactly once, at load, when deriving fx/fy/cx/cy.
2. **Sign flips:** principal_x and principal_y are negated at parse. Not a typo.
3. **Vertical image flip at load:** without it, every overlay is mirrored
   vertically relative to the projection math.
4. **Rotation order R = Rz·Rx·Ry**, applied as `p' = R·p + t` (rotate about the
   model origin, then translate — the model's own origin, *not* an external
   pivot).
5. **Two independent cameras:** background (orthographic, fills viewport) vs
   scene (perspective pinhole replica). Never let them share settings.
6. **View angle depends on image height** — it must be recomputed when frames of
   a different height could appear (the size gate makes all frames equal, so once
   per session after first image is sufficient, but derive it from `h` and `fy`,
   never hardcode).
7. **Window center on resize:** keep the pinned rescale rule, or the overlay
   drifts when the user resizes.
8. **Poses are per (frame, model).**
9. **Clipping ranges scale with focal length** (`0.1·fx → 1.75·fx` for the scene);
   models live at z ≈ −0.25·fy, so the defaults keep everything visible.
10. **The pinhole Z convention:** +Z points *toward* the source, so visible points
    have negative Z and the division by Z in the projection flips lateral signs.
    Combined with the pinned look-direction and window-center signs this is
    self-consistent — changing any one of them breaks alignment.

---

## 10. Acceptance test

Load a UF calibration, an X-ray image of known size w×h, and any STL. Then verify
the projection directly: a vertex at model-local `(X, Y, 0)` on a model at pose
`(x, y, z, 0, 0, 0)` must appear at background pixel

```
sX = ((x + X) / z) · fx + cx
sY = ((y + Y) / z) · fy + cy
```

measured bottom-left, y-up. Verify by placing a small marker model at three known
poses and checking the projected pixel locations against hand-computed values.
That single test catches every sign, flip, and unit bug in this document.
