@echo off
echo Building Whisper GUI - Debug
set LIBCLANG_PATH=C:\Program Files\LLVM\bin
set PATH=C:\Program Files\CMake\bin;%PATH%
set CUDAARCHS=86
set NVCC_PREPEND_FLAGS=--allow-unsupported-compiler
cargo build --features cuda
if %ERRORLEVEL% EQU 0 (
    echo Build successful!
    echo Run with: cargo run
) else (
    echo Build failed!
)
pause
