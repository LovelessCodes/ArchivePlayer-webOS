# First hardware spike

Do this on one real webOS 5+ television before building the full UI. A green
host build proves nothing about the video plane, frame pacing, or the remote.

## Acceptance criteria

- [ ] `.ipk` installs and launches from Developer Mode / Homebrew Channel.
- [ ] SDL2 creates a window and an EGL/GLES2 context at 1920×1080.
- [ ] One animated quad renders at a stable frame time; an on-screen counter
      shows the last frame's milliseconds.
- [ ] D-pad, OK, Back, Play/Pause, FF, RW, and Exit events arrive with the
      expected key codes.
- [ ] Background/foreground (`suspend`/`resume`) preserves the session and
      pauses playback.
- [ ] The SDL window id is exported as `option.windowId`, and
      `com.webos.media` loads an Archive.org MP4 into it.
- [ ] Position polling reports a sensible time; pause, resume, and seek work.
- [ ] The UI overlay stays above the video plane.
- [ ] Killing the app leaves no orphaned media process/plane.

## Traps worth writing down

- The LG SDL fork shifts `SDL_KeyboardEvent` by 4 bytes. Read `state` at +16,
  `wcode` at +20, and `sym` at +24 rather than trusting the upstream layout.
- Set `SDL_WEBOS_ACCESS_POLICY_KEYS_BACK` **before** `SDL_Init` or the Back key
  is swallowed by the system.
- `appinfo.json` with `"type": "native"` cannot be installed through LG Seller
  Lounge; that is expected for a Homebrew client.
- On webOS 5+, `com.webos.media` can use an exported window. webOS 4.x may need
  the ACB plane path; do not assume one code path works on both.
- Keep `DT_NEEDED` small. If a firmware-specific symbol is missing, the app must
  fail gracefully (capability check + message), not fail to load.

## After the spike

Only then implement the screen tree, pairing, library, and detail screens. The
spike's media wrapper should become the only module that knows about
`com.webos.media`.
