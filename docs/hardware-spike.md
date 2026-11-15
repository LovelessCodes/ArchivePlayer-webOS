# Hardware spike: LG 65UN80006LA, webOS 5.4.2

The client was brought up on a real `jhericurl-jebil` set (armv7l, Linux
4.4.84, Rockhopper 5.4.2-2601) through Developer Mode. This is the record of
what the television actually does, so the next port does not rediscover it.

## Acceptance criteria

- [x] `.ipk` installs and launches from Developer Mode.
- [x] SDL2 creates a 1920×1080 full-screen surface.
- [x] The software canvas renders at panel size with stable frame pacing.
- [x] D-pad, OK, and Back events arrive; Back has a private keycode.
- [ ] Background/foreground preserves the session and pauses playback.
- [x] An exported SDL window id is passed as `option.windowId`, and
      `com.webos.media` plays an Archive.org MP4 into it.
- [x] Position advances; pause, resume, and seek work.
- [x] The UI overlays the hardware video plane.
- [x] Killing the app tears down the media pipeline and leaves no orphan.

## What worked

**Install and launch.** `ares-install -d tv` and `ares-launch -d tv` work over
the Developer Mode SSH key. `ares-launch` does not replace a running process:
kill the old PID first (`ps -eo pid,args | grep archiveplayer-webos`) or the new
build never starts.

**Surface.** `set_fullscreen(Desktop)` with no logical size gives a 1920×1080
window and output on this panel; asking for a logical size made SDL letterbox a
1280×720 crop. `SDL_RenderSetLogicalSize` is deliberately left unset.

**Video.** `SDL_webOSCreateExportedWindow(0)` returns `_Window_Id_<n>`. Passing
that id as `payload.option.windowId` to `com.webos.media/load` binds the decoder
to the card. `videooutputd` then reports a full-screen `MAIN` sink:

```text
setWindow() ... [sink:MAIN] source [0,0,640,360] output [0,0,3840,2160]
requestPQset() ... "connectedSource":"VDEC", "fullScreen":true, "width":640,
                   "height":360, "appId":"codes.loveless.archiveplayer"
```

`getForegroundAppInfo` shows the pipeline as `playState: playing` with the
exported `windowId`, and `getActivePipelines` shows `VDEC`/`ADEC` on `card`.

**UI over video.** Two settings are required together:

1. `SDL_VIDEO_EGL_ALLOW_TRANSPARENCY=1` before `SDL_Init`.
2. Clear the SDL window's Wayland opaque region before every present with
   `wl_proxy_marshal(surface, 4, NULL)`.

Without (2) the film decodes correctly but is hidden behind an opaque black
card. `SDL_GetWindowWMInfo` returns the surface; the fork writes a larger
`SDL_SysWMinfo` than its headers declare, so use an oversized buffer and read
the pointers at offset 8 (display) and 12 (surface).

## Traps worth writing down

- **Luna identity.** Register as `com.webos.media.client.<appid>`, not a bare
  name. The installer writes
  `/var/palm/ls2-dev/roles/pub/<appid>.json`, whose `allowedNames` includes that
  wildcard and whose `outbound` grants `com.webos.media`.
  `LSRegister(NULL)` registers but calls return `Message status unknown.`
  `LSCallFromApplication` returns `not privileged (0)`. Both are dead ends.
- **The jail cannot exec `luna-send`.** `spawn failed: Permission denied
  (os error 13)`. Link `libluna-service2` and pump the glib default context.
- **The card is not focused when it is first mapped.** `surface-manager` logs
  `Focused none-group nor grabbed item` and drops every key press even though
  `getForegroundAppInfo` names the app. Call `SDL_RaiseWindow`,
  `SDL_SetWindowInputFocus`, and `SDL_SetWindowGrab` **after the first present**;
  before the surface is mapped SDL accepts the calls and nothing happens. This
  was the difference between a dead remote and a working one.
- **LG's SDL fork puts the raw scancode in `keycode`.** Right arrives as
  `Keycode(79)`, Return as `Keycode(40)`, and the event's `scancode` field is
  empty. Map those values explicitly (`79`→Right, `40`→Return, and so on);
  comparing against `Keycode::Right` (a large `SDLK_` value) never matches.
- **Back is a private keycode, not Escape.** With
  `SDL_WEBOS_ACCESS_POLICY_KEYS_BACK=true` in the environment before
  `SDL_Init`, Back arrives as **482** on webOS 5.4.2. Escape and Backspace
  never fire for it. Without the variable the launcher consumes Back and the
  app goes to the background.
- **`com.webos.service.capture` screenshots do not contain the video plane.**
  `DISPLAY`, `GRAPHIC`, and `ALL` return the same graphics-plane image, with the
  UI over black. `VIDEO` returns `ERROR_NOT_INITIALIZED`. Confirm playback from
  `videooutputd` and from the panel, not from a captured PNG.
- **Remote injection for testing.** From the root shell:

  ```sh
  luna-send -n 1 -f luna://com.webos.service.networkinput/sendSpecialKey \
    '{"key":"RIGHT"}'
  luna-send -n 1 -f luna://com.webos.service.networkinput/test/sendKeyCode \
    '{"keyCode":28}'
  ```

  `sendSpecialKey` accepts `HOME BACK EXIT INFO PLAY PAUSE LEFT RIGHT ENTER`
  and the colour keys. `sendKeyCode` uses Linux input keycodes.
- **`luna-send` needs a TTY.** A plain non-interactive SSH command is a silent
  no-op; use `ssh -tt` with a heredoc.
- **SDL 2.0.10 on webOS 5 lacks `SDL_Metal_DestroyView`.** The ARM build adds a
  no-op shim in `crates/sim/src/getauxval.c`; otherwise the loader exits 127
  before `main`.
- **Pixel order.** `Canvas::pixels()` is straight RGBA; upload it as
  `PixelFormatEnum::ABGR8888`. `RGBA8888` reads alpha as red and turns the UI
  into a red wash.
- **No `ldd`, `pidof`, or `killall` on the set.** Use `ps -eo pid,args` and
  `awk`.
- **A reboot can drop a native app that was launched before it.** On one
  session the package files were present but `listApps` omitted the app,
  `getAppInfo` returned `-101`, and SAM logged
  `APP_LAUNCH_ERR ... creating_item_fail`. Editing the installed
  `appinfo.json` to a native-only manifest and rebooting restored it. The
  fields that are gone are the web-app `iconColor` and `defaultWindowType`, the
  `media` required permission, and relaunch handling; the manifest now sets
  `handlesRelaunch: false` and `requiredMemory: 160`. The packaged fix is
  0.2.1. `opkg` never moved the package past `unpacked`, so do not read its
  status as proof of a usable install.
- **`appinfo.json` with `"type": "native"` cannot be installed through LG Seller
  Lounge.** Developer Mode and the Homebrew Channel are the distribution path.

## Access when SSH is unavailable

The Developer Mode SSH key is the fastest path, but a reboot can leave port 22
closed while the television is still on the network. SSAP remains on 3000
(plain WebSocket) and 3001 (TLS WebSocket). Register with the stored client key,
then use the normal `ssap://` requests:

```text
{"type":"register","payload":{"client-key":"..."}}
{"type":"request","uri":"ssap://com.webos.applicationManager/listApps"}
{"type":"request","uri":"ssap://com.webos.applicationManager/launch",
 "payload":{"id":"codes.loveless.archiveplayer"}}
{"type":"request","uri":"ssap://com.webos.applicationManager/getForegroundAppInfo"}
```

The app list is the useful check: after the native-only manifest fix the client
appears as `codes.loveless.archiveplayer | ArchivePlayer | native`. `launch`
returns `returnValue: true`; the foreground app follows a few seconds later.

`sendSpecialKey` and `sendKeyCode` are **not** in the SSAP allowlist on 5.4.2
(they return 404), so remote injection is different from the root-shell recipe.
Ask for a pointer socket, connect to the returned path, and write the same text
commands the LG remote app uses:

```text
ssap://com.webos.service.networkinput/getPointerInputSocket
  -> {"socketPath":"ws://<tv>:3000/resources/<token>/netinput.pointer.sock"}

type:button
name:RIGHT

type:button
name:ENTER
```

One button command per line is enough; the socket closes after the batch. This
drove Right, Enter, and Back on the real set after SSH was gone. The
`ssap://com.webos.service.networkinput/controls/play` endpoint also works, but it
only covers transport keys.

Screenshots still work through
`ssap://com.webos.service.capture/executeOneShot` with
`{"path":"/tmp/capture.png","method":"GRAPHIC","width":1920,"height":1080,
"format":"PNG"}`. They contain the graphics plane only, so the file size is a
cheap way to see a screen change (the library home was about 308 KB, the player
overlay about 128 KB, and Back returned the 308 KB home image). Retrieving the
file itself still needs SSH; SSAP returns the path and byte count, not the
image.

## Next hardware checks

- Suspend/resume: foreground notification should pause the pipeline and keep
  the media id; resume should re-subscribe, not reload from zero.
- Very long pause: verify the subscription survives the television's idle
  timeout.
- An unsupported file: the server plan should mark it incompatible before the
  player opens; the client should never hand VP9/HEVC to this decoder.
