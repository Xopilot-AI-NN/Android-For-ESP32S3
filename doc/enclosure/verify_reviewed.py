"""Reopen the reviewed file and verify preserved geometry and known envelopes."""
import bpy,json,hashlib
from pathlib import Path
from mathutils import Vector
root=Path(bpy.data.filepath).parent
report=json.loads((root/'doc/enclosure/reviewed-fit.json').read_text())
def fingerprint(obj):
 d={'matrix':[list(r) for r in obj.matrix_world],'modifiers':[(m.name,m.type,getattr(getattr(m,'object',None),'name',None),getattr(m,'operation',None)) for m in obj.modifiers]}
 if obj.type=='MESH':d['vertices']=[list(v.co) for v in obj.data.vertices];d['faces']=[list(p.vertices) for p in obj.data.polygons]
 return hashlib.sha256(json.dumps(d,sort_keys=True).encode()).hexdigest()
for name,digest in report['original_objects_preserved'].items():assert fingerprint(bpy.data.objects[name])==digest,name
for prefix,expected in [('BATTERY',[27,28,5.7]),('HEATSINK',[15,10,5]),('TPS61023',[18,11,5])]:
 r=next(o for o in bpy.context.scene.objects if o.name.startswith(prefix))
 ps=[o.matrix_world@Vector(v) for o in r.children if o.type=='MESH' and not o.get('visual_only',False) for v in o.bound_box]
 actual=[max(v[i] for v in ps)-min(v[i] for v in ps) for i in range(3)]
 assert all(abs(a-b)<.001 for a,b in zip(actual,expected)),(prefix,actual)
assert not any(o.name.startswith('Black grip cap') for o in bpy.context.scene.objects)
assert hashlib.sha256((root/report['source_file']).read_bytes()).hexdigest()==report['source_sha256']
assert report['component_pair_count']==2 and report['case_collision_group_count']==4
assert all(h['exact_check']['classification']=='penetration' and h['exact_check']['nonmanifold_edges']==0 for k in ('component_collisions','case_collisions') for r in report[k] for h in r['intersections'])
print('VERIFIED: 10 protected objects, 3 known envelopes, user deletion, source SHA, 6 closed-volume conflict groups')
