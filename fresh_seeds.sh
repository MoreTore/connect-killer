export DATABASE_URL="postgres://loco:loco@localhost:5432/connect_development"
cd migration && cargo run -- fresh && cd ..
cargo loco task seed_data