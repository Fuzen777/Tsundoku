# Rar archives for the unpacking tests

Rar archives can't be made without WinRAR, so `unpack/tests.rs` uses these real ones. Zip and 7z archives are made by
the tests themselves.

| File | What it is | From |
| --- | --- | --- |
| `rar5-vols.part1.rar` … `part3.rar` | Rar 5 in three parts named the newer way, holding `vols/bigfile.txt` (205,000 bytes, across all three) and `vols/smallfile.txt` (2,050 bytes) | rarfile |
| `rar3-old.rar`, `.r00`, `.r01` | The same files in rar 3, in three parts named the older way | rarfile |
| `rar5-psw.rar` | Rar 5 whose files have the password `password`, names shown | rarfile |
| `rar5-hpsw.rar` | Rar 5 with the password `password`, names hidden too | rarfile |
| `rar3-comment-hpsw.rar` | Rar 3 with the password `password`, names hidden too | rarfile |
| `missing.part1.rar` | The first of two parts, whose second isn't here | unrar (the Rust crate) |

## Licences

From [rarfile](https://github.com/markokr/rarfile)'s `test/files`, under the ISC licence:

```text
Copyright (c) 2005-2024 Marko Kreen <markokr@gmail.com>

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

`missing.part1.rar` is `data/archive.part1.rar` from [unrar.rs](https://github.com/muja/unrar.rs), under the MIT
licence or Apache 2.0, at your choice, as that crate is.
