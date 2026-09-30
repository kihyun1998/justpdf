# Image pixel layout

## The fact
What the pixel buffer `decode_image` returns means — component count, bits per component, color space, the bit packing of masks — varies with the image dictionary and the decode path (DCT/JPX/JBIG2/CCITT/raw). Every place that interprets the buffer as RGB(A) must interpret it by **the same rule**, and that rule must follow `/BitsPerComponent`, array color spaces (Indexed, ICCBased, Separation), `/Decode`, and each path's output shape (e.g. CCITT and JBIG2 give one byte per pixel).

## Why it is cross-cutting
Decoding is in one place, but **interpretation lives in several places, each its own copy**, and none calls another. For CMYK→RGB conversion alone, besides core `cmyk_to_rgb` there are copies in the renderer, SVG, shading and compression. All of them implicitly assume "8-bit, components 1/3/4, named color space". Fixing one copy leaves the rest as they were.

## Territories it holds in
- [Image decoding](../territory/image-decoding.md) — the starting point (`decode_image`, reads only the color space name).
- [Render images](../territory/render-images.md) — `image_to_rgba`, 1-bit mask unpacking (mismatched with the CCITT and JBIG2 output).
- [SVG renderer](../territory/svg-renderer.md) — a copy of `image_to_rgba`, a copy of `cs_from_name`.
- [Render shading](../territory/render-shading.md) — `components_to_color` (a color conversion copy).
- [compress-images](../territory/compress-images.md) — `to_rgb_pixels` (a CMYK copy, f32).
- [compress-grayscale](../territory/compress-grayscale.md) — the luminosity formula (Rec.601; the renderer's masks use Rec.709).
- [Color spaces](../territory/color-spaces.md) — the original `cmyk_to_rgb` and the unused array color space model.

Command to find them again: `rg -n 'fn image_to_rgba|fn to_rgb_pixels|fn components_to_color|fn cs_from_name|fn cmyk_to_rgb' --glob '*.rs' --glob '!target' .`

## What a violation looks like
- An Indexed image renders in the wrong colors, and compression silently skips it.
- 16-bit and 1/2/4-bit raw images show as stripes or cut-off images.
- CCITT and JBIG2 image masks are unpacked wrongly.
- The same image has different colors rendered directly and rendered after compression.
Condition: with 8-bit DeviceRGB/Gray JPEG, all of it is fine — common test inputs meet this condition, so it stays invisible.

## Discovery history
No recorded incident. On 2026-09-23, while writing the map, two research agents (stream/image/color, render) reported the copies independently. All of it is inferred from reading code, and `decode_image` has no unit test.

- Tracked: #42 (render inline images and masks), #46 (image decode color spaces and Decode)

## Where it will recur
**A function that indexes the bytes of a `DecodedImage` or converts color components to RGB is subject to this invariant.** Before writing a new one, find the existing copies with the command above, and use the functions in [Color spaces](../territory/color-spaces.md) where possible. Unless test inputs include Indexed, 16-bit, CMYK and CCITT, this invariant is not checked.
