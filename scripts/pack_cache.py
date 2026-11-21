"""Pack output ownership, locking, and verified disc extraction."""
from contextlib import ExitStack, contextmanager
import json
import os
from pathlib import Path
import tempfile

from global_sound_cache import digest, inventory, lock, owned, replace_tree, write_json

SCHEMA = 2


@contextmanager
def outputs_locked(paths):
    with ExitStack() as locks:
        for path in sorted({str(owned(p)) for p in paths}):
            locks.enter_context(lock(path + ".pack.lock"))
        yield


def _stat(path):
    stat = os.stat(path)
    return [stat.st_size, stat.st_mtime_ns, stat.st_ino, stat.st_ctime_ns]  # ctime: no tool can set it back


def _inventory(tree, memo):
    """global_sound_cache.inventory, through the digest memo when there is one (links still fail closed)."""
    if memo is None:
        return inventory(tree)
    tree = Path(tree)
    rows = []
    for folder, dirs, files in os.walk(tree):
        for name in dirs + files:
            path = Path(folder, name)
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                raise ValueError("cache output contains a link or special file")
        for name in files:
            path = Path(folder, name)
            rows.append([path.relative_to(tree).as_posix(), memo.digest(path)])
    return sorted(rows)


def _same_extraction(identity, key):
    """A receipt from before extraction versions (schema 1, keyed on the bake binary's bytes) describes extract-iso
    version 1's output: it is kept when the image and command match, instead of extracting both discs again."""
    if identity == key:
        return True
    return (isinstance(identity, dict) and identity.get("schema") == 1 and key["extract"] == 1
            and identity.get("image") == key["image"] and identity.get("command") == key["command"])


def extract_image(bake, command, image, destination, run, version=None, fresh=False, memo=None):
    """Old extractors skip same-size files. Only trust a fresh, attested tree.

    No receipt, a changed image or extraction version (`version`: `bake format-versions`' extract-iso), or any
    changed/missing extraction output causes an empty-directory extraction. A pre-existing tree is never attested
    merely because a size-based extractor returned success over it. The bake binary itself is not part of the key: a
    rebuilt bake that extracts the same bytes keeps the discs. The image's digest is remembered by its size, change
    time and inode, so a warm run does not read a multi-GB image again; `fresh` (--rebake) does.
    """
    destination = owned(destination)
    receipt = destination.with_name(destination.name + ".image.json")
    stat = _stat(image)
    old = None
    try:
        old = json.loads(receipt.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        pass
    try:
        known = not fresh and old["image_stat"] == stat
        image_digest = old["identity"]["image"] if known else None
    except (KeyError, TypeError):
        image_digest = None
    if image_digest is None:
        image_digest = digest(image)
    key = dict(schema=SCHEMA, image=image_digest, extract=version, command=command)
    try:
        if _same_extraction(old["identity"], key) and old["files"] and old["files"] == _inventory(destination, memo):
            if old["identity"] != key or old.get("image_stat") != stat:
                _write_receipt(receipt, key, old["files"], stat)
            return False
    except (OSError, ValueError, KeyError, TypeError):
        pass
    receipt.unlink(missing_ok=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="extract-", dir=destination.parent) as temporary:
        stage = Path(temporary, "tree")
        stage.mkdir()
        run([bake, command, image, str(stage)])
        files = _inventory(stage, memo)
        if not files or (_stat(image) != stat and digest(image) != key["image"]):
            raise ValueError("disc extraction incomplete or inputs changed; retry")
        replace_tree(stage, destination)
    _write_receipt(receipt, key, files, stat)
    return True


def _write_receipt(receipt, key, files, stat):
    pending = receipt.with_suffix(".new")
    write_json(pending, dict(identity=key, files=files, image_stat=stat))
    os.replace(pending, receipt)
