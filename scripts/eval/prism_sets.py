"""Download PRiSM's public multilingual phone recognition test sets
(VoxAngeles, Tusom 2021; DoReCo is not public in the collection) from the
Hugging Face collection changelinglab/prism and write each as a
Kaldi-style folder <out>/<set>/ (wav/*.wav, wav.scp, text.good,
text.lang). Prints the collection, each dataset card's opening lines
(licence) and the parquet schema, so a change shows up in the log."""
import io, os, sys
import pyarrow.parquet as pq
import soundfile as sf
from huggingface_hub import get_collection, snapshot_download

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
wanted = ("voxangeles", "tusom")
items = get_collection("changelinglab/prism").items
for it in items:
    print("collection item:", it.item_type, it.item_id)

def pick(names, *candidates):
    for c in candidates:
        for n in names:
            if n.lower() == c:
                return n
    return None

for it in items:
    name = it.item_id.split("/")[-1].lower()
    if it.item_type != "dataset" or not any(w in name for w in wanted):
        continue
    src = snapshot_download(it.item_id, repo_type="dataset", local_dir=os.path.join(out, ".hf", name))
    card = open(os.path.join(src, "README.md"), encoding="utf-8").read().splitlines()
    print(f"--- {it.item_id} card ---\n" + "\n".join(card[:40]))
    set_name = name.removesuffix("-pr")
    dest = os.path.join(out, set_name)
    os.makedirs(os.path.join(dest, "wav"), exist_ok=True)
    scp, text, lang = [], [], []
    for root, _, files in os.walk(src):
        for f in sorted(files):
            if not f.endswith(".parquet"):
                continue
            table = pq.read_table(os.path.join(root, f))
            cols = table.column_names
            print(f"{f}: {table.num_rows} rows, schema: {table.schema}")
            audio_col = pick(cols, "audio")
            id_col = pick(cols, "utt_id", "id", "key", "uttid")
            target_col = pick(cols, "text.good", "text_good", "phones", "phonemes", "ipa", "target", "transcript", "text", "transcription")
            lang_col = pick(cols, "lang_sym", "lang", "language", "text.lang", "lang_id", "iso")
            first = {c: table.column(c)[0].as_py() for c in cols if c != audio_col}
            print("first row (no audio):", first)
            if not (audio_col and target_col):
                sys.exit(f"{it.item_id}: cannot find audio and target columns in {cols}")
            for i in range(table.num_rows):
                row = {c: table.column(c)[i].as_py() for c in (audio_col, id_col, target_col, lang_col) if c}
                utt = str(row[id_col]) if id_col else f"{set_name}_{i:06d}"
                utt = utt.replace(" ", "_")
                audio = row[audio_col]
                data, sr = sf.read(io.BytesIO(audio["bytes"])) if isinstance(audio, dict) and audio.get("bytes") else (audio["array"], audio["sampling_rate"])
                wav = os.path.join("wav", f"{utt}.wav")
                sf.write(os.path.join(dest, wav), data, sr, subtype="PCM_16")
                scp.append(f"{utt} {wav}")
                target = row[target_col]
                text.append(f"{utt} {' '.join(target) if isinstance(target, list) else target}")
                lang.append(f"{utt} {row[lang_col] if lang_col else set_name}")
    for fname, lines in (("wav.scp", scp), ("text.good", text), ("text.lang", lang)):
        open(os.path.join(dest, fname), "w", encoding="utf-8").write("\n".join(lines) + "\n")
    print(f"{set_name}: {len(scp)} utterances written")
