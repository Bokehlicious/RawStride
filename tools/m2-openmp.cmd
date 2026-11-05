@echo off
rem M2 step 3 (approved 2026-11-04): LibRaw with OpenMP vs the baseline, measurement only.
rem Installs libraw[openmp] into vcpkg_installed_openmp\ (separate; the baseline vcpkg_installed\
rem is not touched). Undo: delete vcpkg_installed_openmp\ and target-openmp\. See docs\libraw-build.md
rem Double-click. Log: D:\PhotoWorkflowData\analysis\m2-openmp.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-openmp.log
echo === %DATE% %TIME% === > "%LOG%"
echo [1/2] vcpkg install libraw[openmp] into vcpkg_installed_openmp (first time: several minutes)...
"%VCPKG_ROOT%\vcpkg.exe" install --triplet x64-windows-static-md --x-manifest-root=tools\raw-decode-bench\openmp --x-install-root=vcpkg_installed_openmp >> "%LOG%" 2>&1
if errorlevel 1 (echo vcpkg install FAILED - see %LOG% & pause & exit /b 1)
echo [2/2] build both versions and measure (about 15 minutes) - do not use the computer meanwhile...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0raw-decode-bench\run-openmp.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo measurement FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results under D:\PhotoWorkflowData\analysis\raw-openmp-*  (log: %LOG%)
pause
