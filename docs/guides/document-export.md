# Guide: document and page export

Use this when a document pipeline (a PDF export, an SVG renderer, a report
generator, a slide deck) changes and you must know whether the pages still look
the same. The cross-domain example is a document page: a page whose footer
number moved.

Two routes exist. Raster pages (PNG exports of each page) work on every build.
Direct SVG and PDF comparison needs the `documents` feature. `saccade doctor`
says which one you have. Run the tested blocks with `scripts/test-guides.py`.

## Known good: pages exported twice are identical

```sh case=known-good exit=0
saccade compare samples/doc/v1 samples/doc/same --out pages-ok
```

Name each page export the same in both directories (`page-1.png`, `page-2.png`).
Pages pair by file name; a missing page is reported as missing, not skipped.

## Known bad: the footer number moved

```sh case=known-bad exit=1 says="1 fail"
saccade compare samples/doc/v1 samples/doc/v2 --out pages-bad --metric max --threshold 0.02
```

A page passes when it is equal under the chosen metric and threshold, not
when the document is byte-identical or semantically the same. Text is compared
separately (see below). For a refactor that must be bit-exact use
`saccade prove identity BEFORE AFTER --out proof`.

## Missing input: the second export did not run

```sh case=missing-input exit=2 says="no such directory"
saccade compare samples/doc/v1 samples/doc/not-exported --out pages-missing
```

## Unavailable dependency: SVG and PDF directly

```sh case=unavailable-dependency unavailable=documents exit=2 says="documents"
saccade compare samples/doc/v1.svg samples/doc/v2.svg --dpi 96 --out svg-pages
```

```sh case=known-bad requires=documents exit=1
saccade compare samples/doc/v1.svg samples/doc/v2.svg --dpi 96 --threshold 0.0001 --out svg-pages-bad
```

On a build with `documents` this compares each page at the given density
(`--dpi`, dots per inch, 36 to 600, default 96) and writes
`saccade-documents.v1.json`. See [documents](../documents.md) for limits: static
SVG paths only, no external resources, not a hostile-document sandbox.

## Text, not pixels

For "did the words change", the text route compares strings and positions:
`saccade text a.png b.png --a-source a.json --b-source b.json` imports text
produced elsewhere and works on every build; running OCR needs the `ocr`
feature and a pulled model (`saccade doctor`). See [text](../text.md).
