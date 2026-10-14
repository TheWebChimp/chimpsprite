# Workspace — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

Spec: `docs/superpowers/specs/2026-10-14-workspace-design.md`

## Parte 1: pestañas + Home
1. `DocState` (editor, canvas, timeline, location, saved_version, artboard_name) en `tabs.rs`; la app pasa
   de campos sueltos a `doc: DocState` + `tabs: Vec<Option<DocState>>` + `active: Option<usize>`
   (refactor mecánico `self.editor` → `self.doc.editor`, etc.).
2. Gestión de pestañas pura y con tests: abrir (nueva pestaña, cambiar si ya está abierta, reemplazar
   Untitled intacto), cambiar, cerrar, ir a Home, documentos sin guardar.
3. Barra de pestañas, Home (New/Open/Demo, recientes en escritorio con persistencia de eframe), cerrar con
   cambios, salir con lista de documentos sin guardar, atajos `Ctrl+Tab`/`Ctrl+W`.
4. Autosave web de varias pestañas.

## Parte 2: menú View
Show/hide de extras con `Ctrl+H`, grid configurable por artboard (modelo + `DocCommand::SetGrid` + header
de `.aseprite` + manifest de `.chimp`), Snap to Grid, timeline `Tab`, full screen `F11`, zoom.

## Parte 3: reglas, guías y snap
Modelo `Guide` + `DocCommand::SetGuides`, reglas, crear/mover/borrar guías, lock/clear, snap de
herramientas y de la herramienta Artboard, guardado en `.chimp` y aviso en `.aseprite`.
