"""Add editable component references to the ORIGINAL case, without changing it.
Run: blender -b --disable-autoexec --python-exit-code 1 esp32s3_watch_box.blend --python doc/enclosure/add_components_only.py
"""
from pathlib import Path
import hashlib
import json
import math
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
scene = bpy.context.scene
original = list(scene.objects)

def geometry_record(obj):
    record={'name':obj.name,'matrix':[list(row) for row in obj.matrix_world],
            'hidden':obj.hide_get(),'hide_render':obj.hide_render,
            'collections':sorted(c.name for c in obj.users_collection),
            'modifiers':[(m.name,m.type,getattr(getattr(m,'object',None),'name',None)) for m in obj.modifiers]}
    if obj.type=='MESH':
        record['vertices']=[list(v.co) for v in obj.data.vertices]
        record['faces']=[list(f.vertices) for f in obj.data.polygons]
        record['edges']=[list(e.vertices) for e in obj.data.edges]
    return record

before=[geometry_record(obj) for obj in original]
source_digest=hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest()

def collection(name):
    c=bpy.data.collections.new(name);scene.collection.children.link(c);return c

assembly=collection('COMPONENTS — editable references, original case unchanged')
optional=collection('OPTION — old TP4056, not simultaneous with MCP73871')
clearances=collection('CLEARANCE — card reserve, approximate')

def material(name,color):
    m=bpy.data.materials.new(name);m.diffuse_color=(*color,1);m.use_nodes=True
    m.node_tree.nodes.clear()
    shader=m.node_tree.nodes.new('ShaderNodeBsdfPrincipled')
    output=m.node_tree.nodes.new('ShaderNodeOutputMaterial')
    m.node_tree.links.new(shader.outputs['BSDF'],output.inputs['Surface'])
    shader.inputs['Base Color'].default_value=(*color,1)
    shader.inputs['Roughness'].default_value=.55
    return m

pcb=material('REF PCB blue',(.045,.21,.38))
green=material('REF PCB green',(.08,.34,.20))
metal=material('REF connector metal',(.50,.57,.63))
black=material('REF IC and plastic',(.035,.045,.055))
battery_mat=material('REF battery pouch',(.58,.65,.68))
red=material('REF charger — dimensions assumed',(.70,.18,.16))
glass=material('REF display glass',(.018,.028,.033))
contacts=material('REF contacts',(.65,.42,.09))

def move_to(obj,target):
    for c in list(obj.users_collection):c.objects.unlink(obj)
    target.objects.link(obj)

records=[]
def group(name,location,size,known_xyz,note='',target=assembly):
    obj=bpy.data.objects.new(name,None);target.objects.link(obj);obj.location=location
    obj.empty_display_type='PLAIN_AXES';obj.empty_display_size=2
    obj['nominal_size_mm']=size;obj['known_xyz']=known_xyz;obj['note']=note
    records.append({'name':name,'size_mm':size,'known_xyz':known_xyz,'note':note,'position_mm':list(location)})
    return obj

def box(parent,name,size,offset,mat,target=assembly):
    bpy.ops.mesh.primitive_cube_add(size=1)
    obj=bpy.context.object;obj.name=name;move_to(obj,target)
    obj.parent=parent;obj.location=offset;obj.dimensions=size
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    obj.data.materials.append(mat)
    return obj

def cylinder(parent,name,radius,depth,offset,mat,axis='Z',target=assembly):
    bpy.ops.mesh.primitive_cylinder_add(vertices=32,radius=radius,depth=depth)
    obj=bpy.context.object;obj.name=name;move_to(obj,target)
    obj.parent=parent;obj.location=offset
    if axis=='Y':obj.rotation_euler.x=math.pi/2
    obj.data.materials.append(mat)
    return obj

# Component origins correspond approximately to the existing port cutters.
# All dimensions are in the original file's millimetre coordinates.
display=group('DISPLAY — PCB 48×31, height assumed',(0,0,12.5),[48,31,3],False,'PCB height, glass and active area need verification')
box(display,'Display PCB',(48,31,1),(0,0,.5),pcb)
box(display,'Display glass — visual reference only',(40,27.5,2),(0,0,2),glass)

esp=group('ESP32 + USB-C — 26×18 envelope',(-5,-5.1,4.7),[18,26,3.3],False,'Rotated XY; package heights and exact connector offsets are approximate')
box(esp,'ESP PCB',(18,24,1),(0,1,0.5),pcb)
box(esp,'USB-C metal housing',(8.8,7.2,3.2),(0,-9.4,1.7),metal)
box(esp,'USB-C dark opening',(6.8,.12,1.5),(0,-13.05,1.7),black)
box(esp,'ESP chip — approximate placement',(8,8,.8),(0,2,1.4),black)
box(esp,'ESP antenna keep-out — reference',(15,4,.3),(0,10.5,1.15),black)
for x in [-7.5,7.5]:
    for y in [-8,-5,-2,1,4,7,10]:box(esp,'ESP solder pad',(1.5,1.8,.08),(x,y,1.04),contacts)

sd=group('SD MODULE — 20×18, height assumed',(10,-7.1,5.0),[18,20,3.85],False,'Port alignment and inserted-card projection remain approximate')
box(sd,'SD PCB',(18,20,1),(0,0,.5),pcb)
box(sd,'SD socket metal',(15.2,18,2.4),(0,-1,2.65),metal)
box(sd,'SD socket opening',(13,.15,1.2),(0,-10.05,2.65),black)
reserve=group('SD card reserve +15 mm',(10,-.6,6.9),[18,35,1],False,'Conservative 20+15 envelope requested by user; not an actual card dimension',clearances)
obj=box(reserve,'SD conservative insertion envelope',(18,35,1),(0,0,0),contacts,clearances)
obj.display_type='WIRE';obj.hide_render=True

encoder=group('ENCODER — board 15×17, height assumed',(8.8,8.6,2),[15,17,8.2],False,'Shaft follows existing cutter axis; board/body/shaft dimensions are approximate')
box(encoder,'Encoder PCB',(15,17,1),(0,0,.5),pcb)
box(encoder,'Encoder metal body',(12,11,7),(0,2,4.5),metal)
cylinder(encoder,'Encoder shaft',3,8,(0,10.5,4),metal,'Y')

battery=group('BATTERY — selected 28×27×5.7',(-9,1.5,.9),[28,27,5.7],True,'Placed for inspection; overlaps are not resolved; wire exit and protection unknown')
box(battery,'Battery pouch',(28,27,5.7),(0,0,2.85),battery_mat)
box(battery,'Battery terminal tape',(10,2,.2),(0,12.5,5.8),black)

tps=group('TPS61023 — 18×11×5',(17,-7,.8),[11,18,5],True,'Rotated XY; external envelope known, visual internals approximate')
box(tps,'TPS PCB',(11,18,1),(0,0,.5),green)
box(tps,'TPS inductor',(7,7,4),(0,1,3),black)
box(tps,'TPS controller',(3,3,1),(0,-5,1.5),black)
for y in [-7,7]:
    for x in [-3.5,3.5]:box(tps,'TPS solder pad',(2,2,.08),(x,y,1.04),contacts)

heatsink=group('HEATSINK — 15×10×5',(-5,-3.1,6.6),[15,10,5],True,'Chip alignment is approximate; thermal pad thickness not included')
box(heatsink,'Heatsink base',(15,10,1),(0,0,.5),metal)
for x in [-6.6,-4.4,-2.2,0,2.2,4.4,6.6]:box(heatsink,'Heatsink fin',(1.0,10,4),(x,0,3),metal)

charger=group('MCP73871 — UNMEASURED 32×30×3 placeholder',(0,0,9),[32,30,3],False,'All dimensions assumed; external capacitor NOT included')
box(charger,'MCP73871 PCB — assumed size',(32,30,1),(0,0,.5),red)
box(charger,'MCP73871 IC — visual only',(4,4,1),(0,0,1.5),black)
for x in [-12,-7,7,12]:
    for y in [-12,12]:box(charger,'Charger terminal',(3,3,.1),(x,y,1.05),contacts)

old=group('TP4056 — existing alternative 29×18×5',(45,0,0),[29,18,5],True,'Alternative only; do not treat as a second parallel charger',optional)
box(old,'TP4056 PCB',(29,18,1),(0,0,.5),pcb,optional)
box(old,'TP4056 IC',(5,4,1.5),(0,0,1.75),black,optional)
box(old,'TP4056 USB connector',(8.8,6,4),(-10,0,3),metal,optional)
optional.hide_viewport=True;optional.hide_render=True

# Verify every original object's mesh, placement, visibility, collection and cutters.
# Newly added parts are allowed to collide; resizing the original shell is forbidden.
after=[geometry_record(obj) for obj in original]
assert before==after,'An original object was changed'
body=next(o for o in original if o.name.startswith('CASE —'))
report={'source':'esp32s3_watch_box.blend','original_objects_verified_unchanged':len(original),
        'source_geometry_sha256':source_digest,'case_dimensions_mm':list(body.dimensions),
        'components':records,'fit_verified':False,'warning':'Placement is an editable draft. Parts may overlap each other or the original shell. No case resizing or port changes were made.'}
(ROOT/'doc/enclosure/original-components-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
text=bpy.data.texts.new('READ ME — original case with components')
text.write('''The original case, covers, cutters and fit gauges are unchanged.
New COMPONENTS collection contains editable visual references, grouped by board.
Move a parent empty to move the whole component; expand it for individual parts.
Use Alt+Z X-ray or hide the original CASE to inspect the contents.
Placement is preliminary and includes unresolved overlaps. This is not proof of fit.
Known XYZ: battery 28×27×5.7, TPS61023 18×11×5, heatsink 15×10×5, TP4056 29×18×5.
Other board heights and connector offsets are assumed. MCP73871 dimensions are ALL assumed.
The external charger capacitor has not been modeled because its dimensions are unknown.
The old TP4056 alternative is hidden in its own collection, outside the case.
The SD +15 mm reserve is a wireframe envelope, not a measured card model.
No 12 mm or 30 mm target has been applied to the original case.
''')
# Selection/view settings do not change any original geometry.
bpy.ops.object.select_all(action='DESELECT')
esp.select_set(True);bpy.context.view_layer.objects.active=esp
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.shading.color_type='MATERIAL'
            area.spaces.active.shading.show_xray=True
            area.spaces.active.shading.xray_alpha=.35
            area.spaces.active.region_3d.view_location=(0,0,6)
            area.spaces.active.region_3d.view_distance=92
            area.spaces.active.region_3d.view_rotation=Vector((70,-100,80)).to_track_quat('Z','Y')
bpy.context.view_layer.update()
assert before==[geometry_record(obj) for obj in original]
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'esp32s3_watch_box_components.blend'))
print('ADDED_COMPONENTS='+json.dumps({'groups':len(records),'original_objects_unchanged':len(original),'file':str(ROOT/'esp32s3_watch_box_components.blend')}))
