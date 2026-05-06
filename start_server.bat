@echo off
cd /d C:\Users\18796\Desktop\20260429\CargoNightServer
set DATABASE_URL=postgres://postgres@localhost:5432/cargonight
set JWT_SECRET=cargonight-dev-jwt-secret-key-2026
set SERVER_PORT=8080
set SERVER_HOST=0.0.0.0
set RUST_LOG=debug
target\debug\cargo-night-server.exe > server_debug.log 2>&1
