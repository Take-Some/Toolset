@echo off
setlocal
set CONFIG=Release
if not "%1"=="" set CONFIG=%1

if not exist "..\bin\%CONFIG%\ddsinfo.exe" (
    echo Missing ..\bin\%CONFIG%\ddsinfo.exe
    exit /b 1
)

if not exist ".\release" mkdir ".\release"
copy /Y "..\bin\%CONFIG%\ddsinfo.exe" ".\release\ddsinfo.exe"

echo Wrote .\release\ddsinfo.exe
