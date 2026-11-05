@echo off
rem M2 final build: release build + standalone folder with the last error-handling change (the
rem warning chip for a damaged file whose preview was cut off too), then the broken-file run from
rem the copied folder. Do not use the computer while it runs (about 10 minutes).
rem Results: D:\PhotoWorkflowData\analysis\m2-final\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\m2-final
set LOG=D:\PhotoWorkflowData\analysis\m2-final.log
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/2] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [2/2] broken-file run from the copied folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" -Out "%OUT%" -Runs broken >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
