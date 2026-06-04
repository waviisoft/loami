# Loami brand assets

*Fertile ground for your backend.*

The mark is the "i" of loami: a green sprout with a leaf for its dot, planted in a clod of loam.
The wordmark embeds the identical mark as its final letter.

## Structure

```
brand/
├── svg/            Source vectors (edit these; everything else is derived)
│   ├── loami-mark.svg            sprout + soil, transparent background
│   ├── loami-icon.svg            mark on rounded cream tile (app icon)
│   ├── loami-wordmark.svg        wordmark, light backgrounds
│   ├── loami-wordmark-dark.svg   wordmark, dark backgrounds
│   ├── loami-lockup.svg          icon + wordmark, light backgrounds
│   └── loami-lockup-dark.svg     icon + wordmark, dark backgrounds
├── png/            Raster exports (icons 16–512, wordmark/lockup @1x/@2x)
├── favicon/        Drop-in favicon set + site.webmanifest
└── social/         1280×640 GitHub social preview
```

## Usage

### Favicon (HTML)

```html
<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
```

### mdBook docs site

Copy `favicon/favicon.svg` and `png/loami-icon-32.png` (renamed `favicon.png`) into
`docs/src/theme/` — mdBook picks up `theme/favicon.svg` and `theme/favicon.png` automatically.

### GitHub README (light/dark aware)

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/svg/loami-lockup-dark.svg">
  <img src="brand/svg/loami-lockup.svg" alt="Loami" width="415">
</picture>
```

### GitHub social preview

Upload `social/social-preview.png` in repo Settings → General → Social preview.

## Palette

| Color | Hex | Role |
|---|---|---|
| Soil dark | `#45311F` | Wordmark letters |
| Soil mid | `#6B4A33` | Soil clod, tagline text |
| Soil light | `#A06C48` | Soil highlight |
| Stem | `#4C8C3F` | Sprout stem (the i) |
| Leaf | `#5FA052` | Leaf (the i's dot) |
| Cream | `#F4EBD9` | Icon tile, dark-mode letters, social bg |

## Guidelines

- Use `-dark` variants on dark backgrounds; the soil and greens are shared, only letters change.
- The icon tile is cream — on cream/parchment backgrounds use `loami-mark.svg` (transparent) instead.
- Minimum sizes: icon 16px, wordmark ~100px wide. Don't recolor or re-tilt the leaf.

Type: Nunito Bold (SIL Open Font License), converted to outlines — no font dependency.
