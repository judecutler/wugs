# PHOIBLE phoneme inventories (data, not code)

PHOIBLE collects 3,020 phoneme inventories of 2,186 languages from several databases (UPSID, SAPHON, the Stanford Phonology Archive, Erich Round's Australian inventories and others). `inventories.tsv` keeps one row per inventory that has a Glottocode (3,019):

- `id`: PHOIBLE's InventoryID.
- `glottocode`: the language or dialect.
- `source`: the contributing database (`upsid`, `saphon`, `spa`, `ph`, `er`, `ea`, `ra`, `aa`), as PHOIBLE abbreviates it.
- `name`: the language name as that source gives it.
- `phonemes`: the inventory's phonemes in PHOIBLE's order, space-separated, in NFC. Marginal phonemes are left out.

The wugs desktop app embeds the file. The Sounds panel's **Compared with PHOIBLE** sets the sounds found in a project's transcriptions beside the language's own inventories there and beside its relatives' inventories (its Glottolog family, else its macroarea). Nothing is sent anywhere. Allophones, features, marginal phonemes and the bibliography stay in PHOIBLE: see <https://phoible.org>.

## Source and licence

PHOIBLE is licensed under [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/). This file keeps that licence. It is not covered by the wugs code licence (MPL-2.0) and is bundled alongside the app as a separate work. Cite it as:

> Moran, Steven & McCloy, Daniel (eds.) 2019. PHOIBLE 2.0. Jena: Max Planck Institute for the Science of Human History. https://phoible.org

The file was built from `data/phoible.csv` of [phoible/dev](https://github.com/phoible/dev) at commit `5f82b9c3fbb0b5c630de20e254c5fdad645e3e56` by `tools/data/phoible.py`. The only changes are those listed above: rows without a Glottocode and marginal phonemes dropped, phonemes in NFC, one row per inventory. To update it, run the script on a newer `phoible.csv` and change the commit named here.
