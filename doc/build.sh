#!/bin/sh
mkdir -p build && pdflatex -interaction=batchmode -output-directory=build *.tex
[ -e build/*.pdf ] && mv build/*.pdf ./