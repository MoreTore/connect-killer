# Details

Date : 2024-04-24 22:27:06

Directory /root/connect

Total : 76 files,  12178 codes, 264 comments, 1536 blanks, all 13978 lines

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)

## Files
| filename | language | code | comment | blank | total |
| :--- | :--- | ---: | ---: | ---: | ---: |
| [.cargo/config.toml](/.cargo/config.toml) | TOML | 3 | 0 | 1 | 4 |
| [.devcontainer/devcontainer.json](/.devcontainer/devcontainer.json) | jsonc | 9 | 0 | 0 | 9 |
| [.rustfmt.toml](/.rustfmt.toml) | TOML | 7 | 0 | 1 | 8 |
| [Cargo.lock](/Cargo.lock) | TOML | 5,223 | 2 | 554 | 5,779 |
| [Cargo.toml](/Cargo.toml) | TOML | 51 | 3 | 8 | 62 |
| [examples/playground.rs](/examples/playground.rs) | Rust | 10 | 8 | 5 | 23 |
| [migration/Cargo.lock](/migration/Cargo.lock) | TOML | 4,458 | 2 | 480 | 4,940 |
| [migration/Cargo.toml](/migration/Cargo.toml) | TOML | 16 | 3 | 4 | 23 |
| [migration/src/lib.rs](/migration/src/lib.rs) | Rust | 23 | 0 | 6 | 29 |
| [migration/src/m20220101_000001_users.rs](/migration/src/m20220101_000001_users.rs) | Rust | 43 | 0 | 5 | 48 |
| [migration/src/m20231103_114510_notes.rs](/migration/src/m20231103_114510_notes.rs) | Rust | 29 | 0 | 5 | 34 |
| [migration/src/m20240424_101126_devices.rs](/migration/src/m20240424_101126_devices.rs) | Rust | 53 | 0 | 10 | 63 |
| [migration/src/m20240424_141802_sements.rs](/migration/src/m20240424_141802_sements.rs) | Rust | 76 | 11 | 8 | 95 |
| [migration/src/m20240424_144603_routes.rs](/migration/src/m20240424_144603_routes.rs) | Rust | 56 | 0 | 8 | 64 |
| [migration/src/m20240424_155056_authorized_users.rs](/migration/src/m20240424_155056_authorized_users.rs) | Rust | 61 | 0 | 8 | 69 |
| [migration/src/main.rs](/migration/src/main.rs) | Rust | 5 | 0 | 2 | 7 |
| [minikeyvalue/requirements.txt](/minikeyvalue/requirements.txt) | pip requirements | 3 | 0 | 1 | 4 |
| [src/app.rs](/src/app.rs) | Rust | 76 | 1 | 13 | 90 |
| [src/bin/main.rs](/src/bin/main.rs) | Rust | 7 | 0 | 4 | 11 |
| [src/build.rs](/src/build.rs) | Rust | 24 | 4 | 7 | 35 |
| [src/capnp_mod.rs](/src/capnp_mod.rs) | Rust | 0 | 0 | 1 | 1 |
| [src/common/mkv_helpers.rs](/src/common/mkv_helpers.rs) | Rust | 7 | 0 | 2 | 9 |
| [src/common/mod.rs](/src/common/mod.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/controllers/auth.rs](/src/controllers/auth.rs) | Rust | 110 | 15 | 27 | 152 |
| [src/controllers/connectdata.rs](/src/controllers/connectdata.rs) | Rust | 61 | 3 | 14 | 78 |
| [src/controllers/connectincomming.rs](/src/controllers/connectincomming.rs) | Rust | 66 | 1 | 13 | 80 |
| [src/controllers/mod.rs](/src/controllers/mod.rs) | Rust | 6 | 0 | 1 | 7 |
| [src/controllers/notes.rs](/src/controllers/notes.rs) | Rust | 59 | 0 | 11 | 70 |
| [src/controllers/user.rs](/src/controllers/user.rs) | Rust | 11 | 0 | 4 | 15 |
| [src/controllers/v1.rs](/src/controllers/v1.rs) | Rust | 160 | 9 | 24 | 193 |
| [src/initializers/mod.rs](/src/initializers/mod.rs) | Rust | 2 | 0 | 1 | 3 |
| [src/initializers/view_engine.rs](/src/initializers/view_engine.rs) | Rust | 32 | 0 | 5 | 37 |
| [src/lib.rs](/src/lib.rs) | Rust | 10 | 0 | 1 | 11 |
| [src/mailers/auth.rs](/src/mailers/auth.rs) | Rust | 45 | 13 | 8 | 66 |
| [src/mailers/mod.rs](/src/mailers/mod.rs) | Rust | 1 | 0 | 1 | 2 |
| [src/models/_entities/authorized_users.rs](/src/models/_entities/authorized_users.rs) | Rust | 39 | 3 | 6 | 48 |
| [src/models/_entities/devices.rs](/src/models/_entities/devices.rs) | Rust | 51 | 1 | 7 | 59 |
| [src/models/_entities/mod.rs](/src/models/_entities/mod.rs) | Rust | 7 | 1 | 3 | 11 |
| [src/models/_entities/notes.rs](/src/models/_entities/notes.rs) | Rust | 14 | 1 | 5 | 20 |
| [src/models/_entities/prelude.rs](/src/models/_entities/prelude.rs) | Rust | 6 | 1 | 2 | 9 |
| [src/models/_entities/routes.rs](/src/models/_entities/routes.rs) | Rust | 37 | 1 | 5 | 43 |
| [src/models/_entities/sements.rs](/src/models/_entities/sements.rs) | Rust | 35 | 1 | 4 | 40 |
| [src/models/_entities/users.rs](/src/models/_entities/users.rs) | Rust | 42 | 1 | 6 | 49 |
| [src/models/authorized_users.rs](/src/models/authorized_users.rs) | Rust | 56 | 7 | 14 | 77 |
| [src/models/devices.rs](/src/models/devices.rs) | Rust | 41 | 6 | 8 | 55 |
| [src/models/mod.rs](/src/models/mod.rs) | Rust | 7 | 0 | 1 | 8 |
| [src/models/notes.rs](/src/models/notes.rs) | Rust | 4 | 1 | 3 | 8 |
| [src/models/routes.rs](/src/models/routes.rs) | Rust | 4 | 1 | 2 | 7 |
| [src/models/sements.rs](/src/models/sements.rs) | Rust | 4 | 1 | 2 | 7 |
| [src/models/users.rs](/src/models/users.rs) | Rust | 164 | 79 | 25 | 268 |
| [src/tasks/mod.rs](/src/tasks/mod.rs) | Rust | 1 | 0 | 1 | 2 |
| [src/tasks/seed.rs](/src/tasks/seed.rs) | Rust | 24 | 15 | 6 | 45 |
| [src/views/auth.rs](/src/views/auth.rs) | Rust | 20 | 0 | 4 | 24 |
| [src/views/mod.rs](/src/views/mod.rs) | Rust | 2 | 0 | 1 | 3 |
| [src/views/user.rs](/src/views/user.rs) | Rust | 18 | 0 | 4 | 22 |
| [src/workers/downloader.rs](/src/workers/downloader.rs) | Rust | 34 | 1 | 9 | 44 |
| [src/workers/mod.rs](/src/workers/mod.rs) | Rust | 2 | 0 | 1 | 3 |
| [src/workers/qlog_parser.rs](/src/workers/qlog_parser.rs) | Rust | 49 | 3 | 9 | 61 |
| [tests/mod.rs](/tests/mod.rs) | Rust | 3 | 0 | 1 | 4 |
| [tests/models/authorized_devices.rs](/tests/models/authorized_devices.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/devices.rs](/tests/models/devices.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/mod.rs](/tests/models/mod.rs) | Rust | 5 | 0 | 0 | 5 |
| [tests/models/routes.rs](/tests/models/routes.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/sements.rs](/tests/models/sements.rs) | Rust | 17 | 9 | 6 | 32 |
| [tests/models/users.rs](/tests/models/users.rs) | Rust | 176 | 0 | 48 | 224 |
| [tests/requests/auth.rs](/tests/requests/auth.rs) | Rust | 155 | 7 | 31 | 193 |
| [tests/requests/connectdata.rs](/tests/requests/connectdata.rs) | Rust | 26 | 0 | 4 | 30 |
| [tests/requests/connectincomming.rs](/tests/requests/connectincomming.rs) | Rust | 26 | 0 | 4 | 30 |
| [tests/requests/mod.rs](/tests/requests/mod.rs) | Rust | 7 | 0 | 1 | 8 |
| [tests/requests/notes.rs](/tests/requests/notes.rs) | Rust | 103 | 2 | 19 | 124 |
| [tests/requests/prepare_data.rs](/tests/requests/prepare_data.rs) | Rust | 45 | 1 | 12 | 58 |
| [tests/requests/user.rs](/tests/requests/user.rs) | Rust | 32 | 2 | 7 | 41 |
| [tests/requests/v1.rs](/tests/requests/v1.rs) | Rust | 26 | 0 | 4 | 30 |
| [tests/tasks/mod.rs](/tests/tasks/mod.rs) | Rust | 1 | 0 | 1 | 2 |
| [tests/tasks/seed.rs](/tests/tasks/seed.rs) | Rust | 24 | 15 | 4 | 43 |
| [tests/workers/qlog_parser.rs](/tests/workers/qlog_parser.rs) | Rust | 16 | 2 | 5 | 23 |

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)