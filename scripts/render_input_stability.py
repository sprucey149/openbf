"""Content/stat receipts for alignment inputs and the measuring program.

The complete relevant file catalog is taken twice, including optional sidecar
presence. Symlink directories are followed under their logical registry names;
cycles, unreadable files and changes during an individual read fail closed.
This verifies the supplied artifact roots, not their extraction/bake provenance.
"""
import hashlib
import os
from pathlib import Path

from world_fingerprint import digest

ASSET_SUFFIXES={'.glb','.ron','.ktx2','.json'}
SOURCE_SUFFIXES={'.py','.rs','.wgsl','.toml','.lock','.json'}


def stat_record(value):
    return [value.st_dev,value.st_ino,value.st_mode,value.st_size,value.st_mtime_ns,value.st_ctime_ns]


def file_record(path,relative):
    link_before=path.lstat()
    with path.open('rb') as stream:
        before=os.fstat(stream.fileno());sha=hashlib.sha256()
        for chunk in iter(lambda:stream.read(1024*1024),b''):sha.update(chunk)
        after=os.fstat(stream.fileno())
    link_after=path.lstat()
    if (stat_record(before)!=stat_record(after) or stat_record(after)!=stat_record(path.stat())
            or stat_record(link_before)!=stat_record(link_after)):
        raise ValueError('alignment input changed during read: '+str(path))
    return dict(path=relative,sha256=sha.hexdigest(),stat=stat_record(after),
                link_stat=stat_record(link_after),link=os.readlink(path) if path.is_symlink() else None)


def scan(root,suffixes):
    if not root.is_dir():raise ValueError('alignment input directory absent: '+str(root))
    files=[];links=[]
    def walk(folder,relative,ancestors):
        resolved=folder.resolve(strict=True)
        if resolved in ancestors:raise ValueError('cyclic alignment input directory: '+str(folder))
        for path in sorted(folder.iterdir()):
            key=str(relative/path.name)
            if path.is_dir():
                before=stat_record(path.lstat())
                if path.is_symlink():links.append(dict(path=key,target=os.readlink(path),stat=before))
                walk(path,relative/path.name,ancestors|{resolved})
                if path.is_symlink() and before!=stat_record(path.lstat()):
                    raise ValueError('alignment input directory link changed: '+str(path))
            elif path.suffix.lower() in suffixes:
                files.append(file_record(path,key))
    walk(root,Path(),set())
    return dict(files=files,directory_links=links,sha256=digest([files,links]))


def source_snapshot():
    root=Path(__file__).resolve().parents[1];parts={}
    parts['scripts']=scan(root/'scripts',SOURCE_SUFFIXES)
    engine=root/'engine'
    if (engine/'crates').is_dir():
        parts['engine/crates']=scan(engine/'crates',SOURCE_SUFFIXES)
        for name in ('Cargo.toml','Cargo.lock'):
            if (engine/name).is_file():parts['engine/'+name]=file_record(engine/name,'engine/'+name)
    content=[]
    for prefix,part in parts.items():
        rows=part.get('files',[part])
        content.extend([prefix,row['path'],row['sha256']] for row in rows)
    return dict(parts=parts,content_sha256=digest(content),sha256=digest(parts))


def expected_roots(reference,cases):
    return {'reference':str(Path(reference).resolve()),**{'case:'+name:str(Path(path).resolve()) for name,path in cases}}


def capture(reference,cases):
    roots=expected_roots(reference,cases);records={};errors=[];cache={}
    for name,path in roots.items():
        try:
            if path not in cache:cache[path]=scan(Path(path),ASSET_SUFFIXES)
            records[name]=cache[path]
            if not records[name]['files']:raise ValueError('no alignment input files: '+path)
        except (ValueError,OSError,RuntimeError) as exc:errors.append(str(exc))
    try:source=source_snapshot()
    except (ValueError,OSError,RuntimeError) as exc:source=None;errors.append(str(exc))
    return dict(roots=roots,records=records,source=source,errors=errors)


def finish(before,reference,cases):
    after=capture(reference,cases);same=before==after and not before['errors'] and not after['errors']
    return dict(schema=1,status='pass' if same else 'unmeasured',complete=bool(same),
                method='full_content_and_stat_catalog_v1',before=before,after=after,
                reason=None if same else 'alignment artifact or measuring source changed, or could not be inventoried')


def snapshot_problems(snapshot,roots):
    if (not isinstance(snapshot,dict) or snapshot.get('roots')!=roots or snapshot.get('errors')!=[]
            or not isinstance(snapshot.get('records'),dict) or set(snapshot['records'])!=set(roots)):
        return ['missing complete alignment input catalog']
    source=snapshot.get('source')
    if (not isinstance(source,dict) or not isinstance(source.get('parts'),dict) or 'scripts' not in source['parts']
            or source.get('sha256')!=digest(source['parts'])):return ['invalid measuring source catalog']
    catalogs=list(snapshot['records'].values());content=[]
    for prefix,part in source['parts'].items():
        if not isinstance(prefix,str) or not isinstance(part,dict):return ['invalid source path catalog']
        if 'files' in part:catalogs.append(part);rows=part['files']
        else:
            rows=[part];catalogs.append(dict(files=rows,directory_links=[],sha256=digest([rows,[]])))
        if not isinstance(rows,list) or any(not isinstance(row,dict) or 'path' not in row or 'sha256' not in row for row in rows):
            return ['invalid source content catalog']
        content.extend([prefix,row['path'],row['sha256']] for row in rows)
    if source.get('content_sha256')!=digest(content):return ['source content hash does not cover its catalog']
    for record in catalogs:
        files=record.get('files') if isinstance(record,dict) else None
        links=record.get('directory_links') if isinstance(record,dict) else None
        if (not isinstance(files,list) or not files or not isinstance(links,list)
                or record.get('sha256')!=digest([files,links])):return ['invalid artifact file catalog']
        names=[]
        for row in files:
            if (not isinstance(row,dict) or not isinstance(row.get('path'),str)
                    or Path(row['path']).is_absolute() or '..' in Path(row['path']).parts
                    or not isinstance(row.get('sha256'),str) or len(row['sha256'])!=64
                    or any(not isinstance(row.get(k),list) or len(row[k])!=6 or any(type(v) is not int or v<0 for v in row[k])
                           for k in ('stat','link_stat'))
                    or row.get('link') is not None and not isinstance(row['link'],str)):
                return ['invalid artifact content/stat record']
            names.append(row['path'])
        if len(set(names))!=len(names):return ['duplicate artifact input path']
    return []


def problems(receipt,reference,cases):
    if (not isinstance(receipt,dict) or receipt.get('schema')!=1 or receipt.get('status')!='pass'
            or receipt.get('complete') is not True or receipt.get('method')!='full_content_and_stat_catalog_v1'):
        return ['missing complete unchanged input receipt']
    roots=expected_roots(reference,cases);before=receipt.get('before');after=receipt.get('after')
    errors=snapshot_problems(before,roots)+snapshot_problems(after,roots)
    if errors:return errors
    if before!=after:return ['alignment inputs/source changed during measurement']
    # Reconcile the completed receipt with the CURRENT caller's supplied roots
    # and program too, rather than trusting a stale report from an earlier run.
    current=capture(reference,cases)
    for key in ('roots','records','errors'):
        if current[key]!=after[key]:return ['current alignment inputs differ from the measured catalog']
    if (current['source'] is None or current['source']['content_sha256']!=after['source'].get('content_sha256')):
        return ['current measuring program differs from the receipt']
    return []
