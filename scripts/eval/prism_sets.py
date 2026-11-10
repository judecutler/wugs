"""Download PRiSM's multilingual phone recognition test sets (DoReCo,
VoxAngeles, Tusom 2021) from the Hugging Face collection
changelinglab/prism into <out>/<set>/ as Kaldi-style folders (wav.scp,
text.good, a language file). Prints what it finds, so a change in the
collection shows up in the log."""
import os, sys
from huggingface_hub import get_collection, snapshot_download, list_repo_files

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
wanted = ("doreco", "voxangeles", "tusom")
items = get_collection("changelinglab/prism").items
for it in items:
    print("collection item:", it.item_type, it.item_id)
for it in items:
    name = it.item_id.split("/")[-1].lower()
    if it.item_type != "dataset" or not any(w in name for w in wanted):
        continue
    files = list_repo_files(it.item_id, repo_type="dataset")
    print(f"{it.item_id}: {len(files)} files, e.g. {files[:15]}")
    dest = os.path.join(out, name)
    snapshot_download(it.item_id, repo_type="dataset", local_dir=dest)
    for root, dirs, fs in os.walk(dest):
        depth = root[len(dest):].count(os.sep)
        if depth <= 2 and ".cache" not in root:
            print("  ", root[len(dest):] or "/", sorted(fs)[:8], f"({len(fs)} files)")
