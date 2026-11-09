# Cómo contribuir

Gracias por tu interés. Mirai Shin está en una etapa temprana: el plan está cerrado y se está construyendo la base.

## Licencia de los aportes

Al enviar un aporte (código, documentación, traducciones o recursos gráficos), aceptas que se publique bajo la **GNU AGPL versión 3 con la excepción para el Steamworks SDK** descrita en [`LICENSE-EXCEPTION.md`](LICENSE-EXCEPTION.md), los mismos términos que el resto del proyecto.

Esto es importante: sin la excepción en cada aporte, no se podría compilar la versión de Steam.

## Dependencias

Solo se aceptan dependencias con licencias compatibles: permisivas (MIT, Apache-2.0, BSD) o LGPL enlazada de forma dinámica. Nada de código GPL o AGPL de terceros salvo que tenga una excepción compatible.

## Nombre del producto

El nombre vive solo en [`brand.json`](brand.json). Nunca se escribe a mano en el código ni en los textos de la interfaz.

## Idiomas

Todo texto visible nace en inglés y en español, con claves de traducción.

## Consumo

Si un cambio toca el video, la pausa o el bucle de dibujo, corre `scripts/consumo-pausa.sh` antes de subirlo. Arma un Sway sin pantalla, pone un video de 60 cuadros y falla si en pausa (a mano o con la pantalla apagada) el daemon gasta más de 5 ticks de CPU cada 5 s, o si el límite de cuadros no ahorra. El CI no tiene pantalla ni GPU, así que esto no corre ahí. Necesita `sway`, `ffmpeg` y libmpv.

---

In English: [`CONTRIBUTING.md`](CONTRIBUTING.md).
