#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
: "${PXL:=pxl}"
mkdir -p renders previews
for tool in quipu seeds shantytown bobbin yupana camayoc quechua chaski shuttle rikuy desire-path tambo; do
  "$PXL" validate --strict "source/$tool.pxl"
  "$PXL" render --strict "source/$tool.pxl" --sprite "$tool" --scale 2 -o "renders/$tool.png"
  "$PXL" render --strict "source/$tool.pxl" --sprite "$tool" --scale 4 -o "renders/$tool@2x.png"
  "$PXL" render --strict "source/$tool.pxl" --sprite "$tool-dark" --scale 2 -o "previews/$tool-dark.png"
  "$PXL" render --strict "source/$tool.pxl" --sprite "$tool-light" --scale 2 -o "previews/$tool-light.png"
done
