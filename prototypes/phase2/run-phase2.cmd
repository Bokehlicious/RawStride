@echo off
rem ADR 0008 phase 2: runs P2 (WebView2) and then P3 (wgpu), about 20 minutes in all.
rem Do not use the computer meanwhile: the latency runs press arrow keys and read the screen.
rem Logs: D:\PhotoWorkflowData\analysis\phase2\p2\run-p2.log and ...\p3\run-p3.log
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0p2-webview2\run-p2.ps1"
echo P2 exit %ERRORLEVEL% > D:\PhotoWorkflowData\analysis\phase2\run-phase2.txt
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0p3-wgpu\run-p3.ps1"
echo P3 exit %ERRORLEVEL% >> D:\PhotoWorkflowData\analysis\phase2\run-phase2.txt
echo DONE >> D:\PhotoWorkflowData\analysis\phase2\run-phase2.txt
