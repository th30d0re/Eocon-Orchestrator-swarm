#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

PDFLATEX="${PDFLATEX:-pdflatex}"

echo "Building Constitution.pdf..."
"$PDFLATEX" -interaction=nonstopmode -halt-on-error Constitution.tex
"$PDFLATEX" -interaction=nonstopmode -halt-on-error Constitution.tex
"$PDFLATEX" -interaction=nonstopmode -halt-on-error Constitution.tex

if [ -f Constitution.pdf ]; then
    echo "Build succeeded: Constitution.pdf"
    ls -lh Constitution.pdf
else
    echo "Build failed: Constitution.pdf not found"
    exit 1
fi
