@echo off
rem ADR 0006 step 5, extra run: broken files only (more truncation points and a JPEG named .ARW
rem that LibRaw cannot decode), from the copied standalone folder with a cleaned environment.
rem Uses the existing dist\PhotoWorkflow (no rebuild). About 3 minutes.
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-step5b.log
echo === %DATE% %TIME% === > "%LOG%"
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" -Runs broken >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\step5
echo DONE >> "%LOG%"
pause
