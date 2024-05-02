# Diff Details

Date : 2024-05-02 06:37:49

Directory /root/connect

Total : 52 files,  1246 codes, 220 comments, 182 blanks, all 1648 lines

[Summary](results.md) / [Details](details.md) / [Diff Summary](diff.md) / Diff Details

## Files
| filename | language | code | comment | blank | total |
| :--- | :--- | ---: | ---: | ---: | ---: |
| [Cargo.lock](/Cargo.lock) | TOML | 65 | 0 | 5 | 70 |
| [Cargo.toml](/Cargo.toml) | TOML | 14 | 0 | 1 | 15 |
| [migration/src/lib.rs](/migration/src/lib.rs) | Rust | 0 | 0 | -3 | -3 |
| [migration/src/m20240424_000002_devices.rs](/migration/src/m20240424_000002_devices.rs) | Rust | 60 | 0 | 7 | 67 |
| [migration/src/m20240424_000003_routes.rs](/migration/src/m20240424_000003_routes.rs) | Rust | 68 | 0 | 7 | 75 |
| [migration/src/m20240424_000004_segments.rs](/migration/src/m20240424_000004_segments.rs) | Rust | 84 | 8 | 7 | 99 |
| [migration/src/m20240424_101126_devices.rs](/migration/src/m20240424_101126_devices.rs) | Rust | -53 | 0 | -10 | -63 |
| [migration/src/m20240424_141802_sements.rs](/migration/src/m20240424_141802_sements.rs) | Rust | -76 | -11 | -8 | -95 |
| [migration/src/m20240424_144603_routes.rs](/migration/src/m20240424_144603_routes.rs) | Rust | -56 | 0 | -8 | -64 |
| [migration/src/m20240424_155056_authorized_users.rs](/migration/src/m20240424_155056_authorized_users.rs) | Rust | -61 | 0 | -8 | -69 |
| [migration/src/m20240425_071518_authorized_users.rs](/migration/src/m20240425_071518_authorized_users.rs) | Rust | 61 | 0 | 7 | 68 |
| [src/app.rs](/src/app.rs) | Rust | 29 | 4 | 6 | 39 |
| [src/bin/main.rs](/src/bin/main.rs) | Rust | 0 | 0 | -2 | -2 |
| [src/capnp_mod.rs](/src/capnp_mod.rs) | Rust | 0 | 0 | -1 | -1 |
| [src/controllers/auth.rs](/src/controllers/auth.rs) | Rust | -4 | 4 | 0 | 0 |
| [src/controllers/connectdata.rs](/src/controllers/connectdata.rs) | Rust | 40 | 9 | 8 | 57 |
| [src/controllers/connectincomming.rs](/src/controllers/connectincomming.rs) | Rust | 84 | 5 | 8 | 97 |
| [src/controllers/mod.rs](/src/controllers/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/controllers/useradmin.rs](/src/controllers/useradmin.rs) | Rust | 173 | 63 | 38 | 274 |
| [src/controllers/v1.rs](/src/controllers/v1.rs) | Rust | 4 | 0 | 0 | 4 |
| [src/lib.rs](/src/lib.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/models/_entities/authorized_users.rs](/src/models/_entities/authorized_users.rs) | Rust | 2 | -2 | 0 | 0 |
| [src/models/_entities/authorized_users_old.rs](/src/models/_entities/authorized_users_old.rs) | Rust | 39 | 3 | 6 | 48 |
| [src/models/_entities/devices.rs](/src/models/_entities/devices.rs) | Rust | 4 | 0 | 0 | 4 |
| [src/models/_entities/notes.rs](/src/models/_entities/notes.rs) | Rust | 0 | 0 | -1 | -1 |
| [src/models/_entities/routes.rs](/src/models/_entities/routes.rs) | Rust | 12 | 0 | 1 | 13 |
| [src/models/_entities/segments.rs](/src/models/_entities/segments.rs) | Rust | 53 | 1 | 5 | 59 |
| [src/models/_entities/sements.rs](/src/models/_entities/sements.rs) | Rust | -35 | -1 | -4 | -40 |
| [src/models/devices.rs](/src/models/devices.rs) | Rust | 30 | 0 | 3 | 33 |
| [src/models/routes.rs](/src/models/routes.rs) | Rust | 153 | 56 | 21 | 230 |
| [src/models/segments.rs](/src/models/segments.rs) | Rust | 155 | 61 | 25 | 241 |
| [src/models/sements.rs](/src/models/sements.rs) | Rust | -4 | -1 | -2 | -7 |
| [src/tasks/mod.rs](/src/tasks/mod.rs) | Rust | 1 | 0 | -1 | 0 |
| [src/tasks/seed_from_mkv.rs](/src/tasks/seed_from_mkv.rs) | Rust | 75 | 6 | 19 | 100 |
| [src/views/auth.rs](/src/views/auth.rs) | Rust | 10 | 1 | 3 | 14 |
| [src/views/mod.rs](/src/views/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/views/route.rs](/src/views/route.rs) | Rust | 10 | 2 | 5 | 17 |
| [src/websockets/handler.rs](/src/websockets/handler.rs) | Rust | 60 | 2 | 13 | 75 |
| [src/websockets/mod.rs](/src/websockets/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/workers/jpg_extractor.rs](/src/workers/jpg_extractor.rs) | Rust | 20 | 1 | 5 | 26 |
| [src/workers/mod.rs](/src/workers/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/workers/qlog_parser.rs](/src/workers/qlog_parser.rs) | Rust | 147 | 7 | 11 | 165 |
| [tests/models/authorized_devices.rs](/tests/models/authorized_devices.rs) | Rust | -17 | -9 | -6 | -32 |
| [tests/models/authorized_users.rs](/tests/models/authorized_users.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/segments.rs](/tests/models/segments.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/sements.rs](/tests/models/sements.rs) | Rust | -17 | -9 | -6 | -32 |
| [tests/requests/mod.rs](/tests/requests/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [tests/requests/useradmin.rs](/tests/requests/useradmin.rs) | Rust | 26 | 0 | 4 | 30 |
| [tests/tasks/mod.rs](/tests/tasks/mod.rs) | Rust | 2 | 0 | 0 | 2 |
| [tests/tasks/seed_data.rs](/tests/tasks/seed_data.rs) | Rust | 16 | 0 | 5 | 21 |
| [tests/tasks/seed_from_mkv.rs](/tests/tasks/seed_from_mkv.rs) | Rust | 16 | 0 | 5 | 21 |
| [tests/workers/jpg_extractor.rs](/tests/workers/jpg_extractor.rs) | Rust | 16 | 2 | 5 | 23 |

[Summary](results.md) / [Details](details.md) / [Diff Summary](diff.md) / Diff Details