# API surface used by this client

The server is the source of truth. This page is a short index; the full contract
is in the main repository's
[docs/tv-clients.md](https://github.com/LovelessCodes/ArchivePlayer/blob/main/docs/tv-clients.md).

| Method | Path | Auth | Used for |
| --- | --- | --- | --- |
| `GET` | `/api/health` | none | Version and `apiVersion` guard. |
| `GET` | `/api/auth/status` | optional bearer | Current user / registration state. |
| `POST` | `/api/device/start` | none | Start pairing; returns `userCode` and QR URL. |
| `POST` | `/api/device/token` | device code | Poll approval and redeem the bearer token. |
| `GET` | `/api/library` | bearer | Categories and titles. |
| `GET` | `/api/library/continue-watching` | bearer | Resume row. |
| `POST` | `/api/library` | bearer + `library:write` | Add an Archive.org identifier. |
| `GET` | `/api/search?q=&page=` | bearer | Archive.org search. |
| `GET` | `/api/playback/{identifier}?platform=webos&file=` | bearer | Details, files, episodes, and `playbackPlan`. |
| `PUT` | `/api/playback/{identifier}/progress` | bearer | Save position/duration. |
| `POST` | `/api/playback/{identifier}/completed` | bearer | Mark a file/title complete. |

## Pairing

```json
{
  "clientName": "Living room TV",
  "platform": "webos",
  "scopes": ["library:read", "library:write", "progress:write"]
}
```

Poll with `{ "deviceCode": "..." }` every `interval` seconds. While pending the
server returns HTTP 400 `authorization_pending`; HTTP 429 `slow_down` means back
off. After approval the response is:

```json
{
  "token": "<opaque bearer token>",
  "expiresAt": 1809948069,
  "user": { "id": 1, "email": "owner@example.com", "displayName": "Owner" }
}
```

Store the token in the private file store, never in browser storage. Send it as
`Authorization: Bearer <token>` on every authenticated request.

## Search and add

Search is server-side, so the TV never calls Archive.org directly:

```http
GET /api/search?q=night&page=1
Authorization: Bearer <token>
```

Each result carries `identifier`, `title`, `mediaType`, and `thumbnailUrl`.
Adding one is a small JSON body:

```http
POST /api/library
Authorization: Bearer <token>
Content-Type: application/json

{ "identifier": "night_of_the_living_dead", "categoryName": "Uncategorized" }
```

The response is the new `LibraryItem`. A duplicate is `HTTP 409` with
`code: "conflict"`; refresh the library rather than treating it as a failure.
The session needs `library:write`; download and playback only need
`library:read`/`progress:write`.

## Playback plan

For a movie:

```http
GET /api/playback/night_of_the_living_dead?platform=webos
```

For an episode or a specific quality:

```http
GET /api/playback/the_twilight_zone?platform=webos&file=The.Twilight.Zone.S01E01.mp4
```

`playbackPlan` is `null` when no video exists. `compatible: false` is a
deliberate answer: show `reason` rather than trying a decoder the TV is not
expected to have. `videoFileUrls` also carries `format`, `mimeType`, `sizeBytes`,
`lengthSeconds`, `width`, `height`, and `videoCodec` for a manual picker.

`durationSeconds` on `MediaDetails` is the saved player duration, not the
Archive.org file length; use `playbackPlan.lengthSeconds` before playback.

## Progress payloads

```json
PUT /api/playback/night_of_the_living_dead/progress
{ "fileName": "night.mp4", "positionSeconds": 1284, "durationSeconds": 5760 }
```

```json
POST /api/playback/night_of_the_living_dead/completed
{ "fileName": "night.mp4", "itemCompleted": true }
```

Use the `fileName` from `playbackPlan`, not the title. For a series episode send
`itemCompleted: false`; the server records the episode and leaves the series
open. Progress always wins over a guessed duration: use the plan length.

## Artwork

The server returns absolute `thumbnailUrl`, `tmdbPosterUrl`, and
`tmdbBackdropUrl` values. The client fetches them directly from the public host,
not through the server API. Private and loopback hosts are refused, responses
are capped at 8 MiB, and redirect targets are checked again after the request.

Two derived bitmaps are kept per title: a 360×540 poster and a 960×540
backdrop. Only portrait sources at least 240 px wide become posters, and only
wide sources at least 960 px wide (roughly 16:9) become backdrops. Archive.org's
180×101 item image therefore never gets stretched over the hero. A failed
download, redirect, or aspect guard is silent: the procedural artwork stays.

## Failure mapping

| Server response | Client action |
| --- | --- |
| `401` | Clear token, keep server, return to pairing. |
| `403` | Show "this device lacks permission"; do not re-pair silently. |
| `429` + `Retry-After` | Honor the delay; for pairing, increase the poll interval. |
| `5xx` | Show a retry affordance; keep the token. |
| transport error | Try the remote URL if configured; otherwise show "server unreachable". |
