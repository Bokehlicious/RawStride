@echo off
rem ADR 0006 step 1: LibRaw with OpenMP linked into the app (not used yet).
rem  1. vcpkg install (root vcpkg.json now asks for libraw[openmp])
rem  2. cargo test -p pw-libraw (pixel-identical regions on the test RAWs)
rem  3. standalone distribution folder (dist\PhotoWorkflow) with the runtime DLLs
rem  4. start-up and idle memory: M1 reference vs step 1
rem Double-click. Log: D:\PhotoWorkflowData\analysis\m2-step1.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-step1.log
set OUT=D:\PhotoWorkflowData\analysis\step1
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/4] vcpkg install libraw[openmp]...
"%VCPKG_ROOT%\vcpkg.exe" install --triplet x64-windows-static-md >> "%LOG%" 2>&1
if errorlevel 1 (echo vcpkg FAILED - see %LOG% & pause & exit /b 1)
copy /y vcpkg_installed\vcpkg\status "%OUT%\vcpkg-status.txt" >> "%LOG%"
echo [2/4] cargo test -p pw-libraw...
set PW_TESTDATA=D:\PhotoWorkflowData\RAW
cargo test --release -p pw-libraw > "%OUT%\test-pw-libraw.txt" 2>&1
if errorlevel 1 (type "%OUT%\test-pw-libraw.txt" >> "%LOG%" & echo tests FAILED - see %OUT%\test-pw-libraw.txt & pause & exit /b 1)
echo [3/4] release build + distribution folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo packaging FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [4/4] start-up and memory, M1 reference vs step 1 (the app window opens and closes 10 times)...
if exist "%OUT%\startup.csv" del "%OUT%\startup.csv"
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe D:\PhotoWorkflowData\builds\m1-reference\photoworkflow.exe -Label m1 -Csv "%OUT%\startup.csv" >> "%LOG%" 2>&1
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe dist\PhotoWorkflow\photoworkflow.exe -Label step1 -Csv "%OUT%\startup.csv" >> "%LOG%" 2>&1
if errorlevel 1 (echo measurement FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
