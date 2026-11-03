# Animated Fluent Emoji

Original animated PNGs (APNG), © Microsoft Corporation, from the official
[microsoft/fluentui-emoji-animated](https://github.com/microsoft/fluentui-emoji-animated)
repository. The MIT license is included in `LICENSE.txt`.

These are complete frame-by-frame animations, not static PNG previews.
The original artwork, transparency, and frame timings are preserved.
Files are bundled locally and require no network connection at runtime.

`sources.json` records the upstream tree revision and original path of every
file. Update the assets with `python3 scripts/import_fluent_animations.py`.

Compatible inline emoticons prefer these animations; existing resources remain
available for emoji that have no corresponding official animated Fluent asset.
Winks use the same local artwork at conversation-filling size and play
automatically on sending or receiving a new wink.
