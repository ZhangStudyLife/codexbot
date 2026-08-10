@echo off
setlocal
chcp 65001 >nul

set "CODEXBOT_ROOT=%~dp0"
set "CODEXBOT_UI=%CODEXBOT_ROOT%ui"
set "CODEXBOT_DIST=%CODEXBOT_ROOT%dist"
set "CODEXBOT_RELEASE=%CODEXBOT_UI%\src-tauri\target\release"

where cargo.exe >nul 2>&1
if errorlevel 1 (
  echo [ERROR] 未找到 Rust Cargo，请先安装 Rust 工具链。
  exit /b 1
)

where pnpm.cmd >nul 2>&1
if errorlevel 1 (
  echo [ERROR] 未找到 pnpm，请先安装 pnpm。
  exit /b 1
)

echo [1/3] 安装并校验前端依赖...
pushd "%CODEXBOT_UI%"
call pnpm.cmd install --frozen-lockfile
if errorlevel 1 goto :fail

echo [2/3] 构建 CodexBot Windows 安装包...
call pnpm.cmd desktop:build
if errorlevel 1 goto :fail

echo [3/3] 整理 Windows 交付文件...
if not exist "%CODEXBOT_DIST%" mkdir "%CODEXBOT_DIST%"
if errorlevel 1 goto :fail
copy /Y "%CODEXBOT_RELEASE%\bundle\nsis\CodexBot_0.1.0_x64-setup.exe" "%CODEXBOT_DIST%\CodexBot-Setup-0.1.0-x64.exe" >nul
if errorlevel 1 goto :fail
copy /Y "%CODEXBOT_RELEASE%\codexbot-desktop.exe" "%CODEXBOT_DIST%\CodexBot-Portable-0.1.0-x64.exe" >nul
if errorlevel 1 goto :fail

echo.
echo [OK] Windows 软件已生成：
echo %CODEXBOT_DIST%\CodexBot-Setup-0.1.0-x64.exe
echo %CODEXBOT_DIST%\CodexBot-Portable-0.1.0-x64.exe
popd
exit /b 0

:fail
popd
echo.
echo [ERROR] Windows 安装包构建失败，请保留上方错误信息。
exit /b 1
