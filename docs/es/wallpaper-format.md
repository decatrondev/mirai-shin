# Formato de fondo (v1)

> En desarrollo: la app todavía no dibuja fondos. Este formato es el que hoy lee la biblioteca.

Un fondo es una **carpeta** dentro de la carpeta de fondos, con un `manifest.json` y sus archivos.

```
wallpapers/
  atardecer/
    manifest.json
    media/loop.mp4
    preview.webp
```

La carpeta de fondos es, por defecto, `~/.local/share/net.decatron.MiraiShin/wallpapers/` en Linux y `%APPDATA%\net.decatron.MiraiShin\wallpapers\` en Windows. Se cambia con `wallpapers_dir` en los ajustes.

## `manifest.json`

```json
{
  "schema_version": 1,
  "id": "atardecer",
  "type": "video",
  "file": "media/loop.mp4",
  "title": "Atardecer",
  "preview": "preview.webp",
  "author": "Ana",
  "source_url": "https://example.com/atardecer",
  "tags": ["tarde", "calma"]
}
```

| Campo | Obligatorio | Detalle |
|---|---|---|
| `schema_version` | sí | `1` |
| `id` | sí | Texto único. Si dos carpetas tienen el mismo id, gana la primera (por nombre de carpeta) y la otra queda como `duplicate_id`. |
| `type` | sí | `image` o `video` |
| `file` | sí | Ruta relativa a la carpeta. Imágenes: `png`, `jpg`, `jpeg`, `webp`. Videos: `mp4`, `webm`, `mkv`. |
| `title` | sí | Texto |
| `preview` | no | Imagen relativa a la carpeta (`png`, `jpg`, `jpeg`, `webp`) |
| `author`, `source_url` | no | Texto |
| `tags` | no | Lista de textos |

Reglas:
- Las rutas no pueden salir de la carpeta: se rechazan las absolutas, las letras de unidad y `..` (`unsafe_path`). No se siguen enlaces simbólicos.
- Los campos desconocidos se ignoran: un fondo hecho con una versión más nueva abre igual en una vieja.
- Un `schema_version` mayor que 1 se lee igual y se marca como hecho con una versión más nueva.
- Las carpetas cuyo nombre empieza con `.` se saltan.

## Problemas

Un fondo inválido queda en la biblioteca con todos sus problemas, nunca se esconde. Cada problema es un código, a veces seguido del campo (`missing_field:title`):

| Código | Qué significa |
|---|---|
| `missing_manifest` | La carpeta no tiene `manifest.json` |
| `unreadable_manifest` | `manifest.json` existe pero no se pudo leer |
| `invalid_json` | No es JSON válido |
| `not_an_object` | El JSON no es un objeto `{ … }` |
| `missing_field` | Falta un campo obligatorio |
| `wrong_type` | Un campo tiene otro tipo (por ejemplo, un número en vez de texto) |
| `empty_field` | Un campo de texto está vacío |
| `unsupported_schema` | `schema_version` es menor que 1 |
| `unknown_kind` | `type` no es `image` ni `video` |
| `unsafe_path` | Una ruta sale de la carpeta |
| `file_not_found` | El archivo no existe en la carpeta |
| `extension_mismatch` | La extensión no corresponde al tipo |
| `duplicate_id` | Otra carpeta ya usa este id |
