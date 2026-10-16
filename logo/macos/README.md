# justpdf macOS app icons

- Default: [justpdf-macos-light.icns](justpdf-macos-light.icns).
- Alternative: [justpdf-macos-dark.icns](justpdf-macos-dark.icns). These are manually selected alternatives; the ICNS files do not switch automatically with system appearance.
- The matching 1024 px PNG files are transparent static masters. Each matching `.iconset/` contains all ten 1x/2x representations for 16, 32, 128, 256, and 512 points.
- [Preview](justpdf-macos-preview.png) includes both appearances and native 16, 32, and 64 px images.

## Workflow and geometry

These are static, legacy-compatible macOS icons, not Icon Composer input layers. Use the ICNS for an app bundle's icon resource or copy the image into Finder's Get Info icon field for a local custom icon. Integrate the iconset through the consuming app's build system where applicable.

The original symbol is fitted without distortion inside a monochrome rounded-square container. Geometry is a local continuous-corner approximation: 832 px container centered on a 1024 px canvas, 96 px transparent outer margins, and a 570 px symbol bounding box. Apple's legacy production-template download returned HTTP 403 during this run, so exact Apple-template conformance is not claimed. No decorative shadow, gradient, or texture was added.

References: [Apple Design Resources](https://developer.apple.com/design/resources/), [App icon guidelines](https://developer.apple.com/design/human-interface-guidelines/app-icons), and [iconset representation requirements](https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Optimizing/Optimizing.html).

## Validation and rebuilding

Validated all ten iconset filenames, dimensions, and transparent corners. Each ICNS contains 16, 32, 64, 128, 256, 512, and 1024 px representations, independently decoded with Pillow and compared pixel-for-pixel with the corresponding PNGs. Both appearances were visually reviewed. Finder/Dock installation was not tested because this pack was generated on Windows.

Rebuild from the repository root with `python tools/build_macos_icons.py justpdf` (Pillow and resvg-py required). Validate without rebuilding with `python tools/build_macos_icons.py justpdf --validate-only`.
