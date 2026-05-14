<p align="center">
  <img src="assets/Ghost.svg" alt="Logo" width="200">
</p>

# GhostLayer

GhostLayer is a fairly small Rust library for creating PDF documents with invisible yet selectable and searchable text.
There are basically just two modes with this library:

- Create a new PDF from a bunch of images
- Overlay an existing PDF with some text

Both require a JSON per Page for a description on where/what text is located.

## What is the JSON format ?

I really dont want to write down how the JSON format is structured.
For one, I am lazy and for two it may change whenever I feel like it.

## Purpose

This Rust library is not innovative. \
It just solves a problem I had; I want [tesseract](https://github.com/tesseract-ocr/tesseract)/[OCRmyPDF](https://github.com/ocrmypdf/ocrmypdf) kind of PDF overlaying but in a way that I can get it in my `GhostNode` iOS/macOS app.
Because I can't reasonably put an entire python runtime in an iOS/macOS app, this library does what I want instead.

## AI usage

I don't know how to make a good (enough) FFI interface for accessing a rust library from a swift iOS/macOS app, so I had Claude write me the FFI.
Also, I am lazy and didn't want to write the tests myself, so I had Claude write them too. 

## Copyright Notice

I've made a dedicated [`NOTICE`](NOTICE) file (okay, I had Claude write one), but I want to also have it in the `README`:

This app is not innovative and didn't invent this overlaying concept! \
This is basically just a Rust copy of [tesseract](https://github.com/tesseract-ocr/tesseract)s [pdfrenderer.cpp](https://github.com/tesseract-ocr/tesseract/blob/96772c5761cf2407798f97a513a68993aea083c8/src/api/pdfrenderer.cpp).
I also just plainly copied their `pdf.tff`!
So all rights regarding that algorithm, font file and font embedding strategy belong to them or wherever they've found it!

The Ghost icon at assets/Ghost.svg is the sole property of my sister Sina Friedrich and not covered by the Apache licence. 

As for the test images, I've just opened my browser, typed in [commons.wikimedia.org](https://commons.wikimedia.org), and searched for images with text and a "No restrictions" licence; which means they are not covered by my Apache licence!
Should you be the owner of any of these pictures and disagree with me using them, know: they are also available on Wikimedia Commons.
I'll remove them from this repo if you want, tho.

## Contributions

Sure, make some PRs for this repo, but I don't see why you should.
Maybe if someone with more swift-Rust-FFI knowledge knows how to improve that part ?

## Whats with the Name ?

Ghosts are (mostly) invisible but still there(?) and this puts an invisible (yet existent) text layer on top of PDF documents.
Also, I wrote an iOS app called `GhostNode` using this library and my sister drew a very cute ghost icon for it.
