"""Pack output ownership, locking, and verified disc extraction."""
from contextlib import ExitStack, contextmanager
import json
import os
from pathlib import Path
import tempfile

from global_sound_cache import digest, inventory, lock, owned, replace_tree, write_json


@contextmanager
def outputs_locked(paths):
    with ExitStack() as locks:
        for path in sorted({str(owned(p)) for p in paths}):
            locks.enter_context(lock(path + ".pack.lock"))
        yield


def extract_image(bake, command, image, destination, run):
    """Old extractors skip same-size files. Only trust a fresh, attested tree.

    No receipt, a changed image/tool, or any changed/missing extraction output
    causes an empty-directory extraction. A pre-existing tree is never attested
    merely because a size-based extractor returned success over it.
    """
    destination = owned(destination)
    receipt = destination.with_name(destination.name + ".image.json")
    key = dict(schema=1, image=digest(image), bake=digest(bake), command=command)
    try:
        old = json.loads(receipt.read_text(encoding="utf-8"))
        if old["identity"] == key and old["files"] and old["files"] == inventory(destination):
            return False
    except (OSError, ValueError, KeyError, TypeError):
        pass
    receipt.unlink(missing_ok=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="extract-", dir=destination.parent) as temporary:
        stage = Path(temporary, "tree")
        stage.mkdir()
        run([bake, command, image, str(stage)])
        files = inventory(stage)
        if not files or digest(image) != key["image"] or digest(bake) != key["bake"]:
            raise ValueError("disc extraction incomplete or inputs changed; retry")
        replace_tree(stage, destination)
    pending = receipt.with_suffix(".new")
    write_json(pending, dict(identity=key, files=files))
    os.replace(pending, receipt)
    return True
