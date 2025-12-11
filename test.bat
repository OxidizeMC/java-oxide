@echo off

cargo build -p java-oxide-gen
if %errorlevel% neq 0 exit /b %errorlevel%

pushd gen-test
set "RUST_BACKTRACE=1"

@REM ..\target\debug\java-oxide-gen.exe -vv generate
..\target\debug\java-oxide-gen.exe -vv generate >jog.txt 2>&1
if %errorlevel% neq 0 exit /b %errorlevel%

cargo fmt -- --config-path ..\rustfmt.toml --config "max_width=200"
if %errorlevel% neq 0 exit /b %errorlevel%

cargo check >cargo-check-output.txt 2>&1
if %errorlevel% neq 0 exit /b %errorlevel%

set "RUST_BACKTRACE="
popd
