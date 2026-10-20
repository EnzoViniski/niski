@echo off
echo.
echo  Medza Agents - Setup e Build Automatizado
echo  =========================================
echo.
echo  Este script vai instalar todas as dependencias e gerar o instalador .exe.
echo  Pode levar 20-30 minutos na primeira execucao.
echo.
pause

powershell -ExecutionPolicy Bypass -File "%~dp0scripts\setup.ps1"

echo.
pause
