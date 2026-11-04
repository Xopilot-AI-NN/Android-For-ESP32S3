"""Extract small mechanical checks from the unchanged original case.

Blender background only. STL coordinates are millimetres, independent of scene
unit display settings. These checks reproduce current openings, not proposed fixes.
"""
from pathlib import Path
import hashlib
import json
import struct
import bpy
import bmesh
from mathutils import Matrix, Vector

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'doc/enclosure/print_checks'
OUT.mkdir(exist_ok=True)
source_scene=bpy.context.scene
sources=list(source_scene.objects)

def signature(objects):
    rows=[]
    for o in objects:
        rows.append({'name':o.name,'matrix':[list(row) for row in o.matrix_world],
                     'verts':[list(v.co) for v in o.data.vertices] if o.type=='MESH' else [],
                     'faces':[list(p.vertices) for p in o.data.polygons] if o.type=='MESH' else [],
                     'cutters':[(m.name,m.type,getattr(getattr(m,'object',None),'name',None)) for m in o.modifiers]})
    return hashlib.sha256(json.dumps(rows,sort_keys=True).encode()).hexdigest()

original_signature=signature(sources)
collection=bpy.data.collections.new('FIT COUPONS — nominal millimetres')
source_scene.collection.children.link(collection)

def world_copy(source,name):
    deps=bpy.context.evaluated_depsgraph_get()
    mesh=bpy.data.meshes.new_from_object(source.evaluated_get(deps))
    mesh.transform(source.matrix_world)
    obj=bpy.data.objects.new(name,mesh);collection.objects.link(obj)
    return obj

def intersect_box(obj,minimum,maximum):
    bpy.ops.mesh.primitive_cube_add(size=1,location=[(a+b)/2 for a,b in zip(minimum,maximum)])
    tool=bpy.context.object;tool.dimensions=[b-a for a,b in zip(minimum,maximum)]
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    modifier=obj.modifiers.new('Extract original wall only','BOOLEAN')
    modifier.operation='INTERSECT';modifier.solver='EXACT';modifier.object=tool
    bpy.context.view_layer.objects.active=obj
    bpy.ops.object.modifier_apply(modifier=modifier.name)
    bpy.data.objects.remove(tool,do_unlink=True)

checks=[]
def finish(obj,filename,description,flat_side=False):
    if flat_side:
        # Lay the outside wall on the build plate. This rigid rotation preserves
        # every opening; it is not a scale or a change to the source case.
        for vertex in obj.data.vertices:
            x,y,z=vertex.co
            vertex.co=(x,-z,y)
    points=[v.co for v in obj.data.vertices]
    low=Vector([min(p[i] for p in points) for i in range(3)])
    for vertex in obj.data.vertices:vertex.co-=low
    obj.data.update()
    bm=bmesh.new();bm.from_mesh(obj.data)
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
    nonmanifold=sum(not e.is_manifold for e in bm.edges)
    volume=abs(bm.calc_volume())
    assert nonmanifold==0,(filename,nonmanifold)
    assert volume>0,filename
    bm.to_mesh(obj.data);bm.free()
    mesh=obj.data;mesh.calc_loop_triangles()
    with (OUT/filename).open('wb') as f:
        header=('Watch original case fit check; units mm; '+filename).encode()[:80]
        f.write(header.ljust(80,b'\0'));f.write(struct.pack('<I',len(mesh.loop_triangles)))
        for triangle in mesh.loop_triangles:
            verts=[mesh.vertices[i].co for i in triangle.vertices]
            normal=(verts[1]-verts[0]).cross(verts[2]-verts[0]).normalized()
            f.write(struct.pack('<12fH',*normal,*verts[0],*verts[1],*verts[2],0))
    size=[max(v.co[i] for v in mesh.vertices) for i in range(3)]
    checks.append({'file':filename,'description':description,'dimensions_mm':[round(v,4) for v in size],
                   'volume_mm3':round(volume,3),'nonmanifold_edges':nonmanifold,
                   'sha256':hashlib.sha256((OUT/filename).read_bytes()).hexdigest()})
    obj['test_description']=description
    obj['stl_units']='mm'
    obj['source_case_unchanged']=True
    return obj

body=next(o for o in sources if o.name.startswith('CASE —'))
front=next(o for o in sources if o.name.startswith('NEW FRONT COVER'))
gauge=next(o for o in sources if o.name.startswith('PCB 4-hole fit gauge'))

bezel=finish(world_copy(front,'01 Display bezel — original aperture'),'01_display_bezel.stl',
             'Original front cover: compare visible area, perimeter seating and mounting holes with the real display')
plate=finish(world_copy(gauge,'02 Display hole gauge — original'),'02_display_hole_gauge.stl',
             'Original 48x31 PCB gauge: compare all four mounting-hole centres against the real PCB')
ports=world_copy(body,'03 USB and SD wall — original openings')
intersect_box(ports,(-23.8,-18.5,0),(23.8,-14.5,12.5))
ports=finish(ports,'03_usb_sd_wall.stl','Original USB/SD wall: compare both ports simultaneously, including their relative spacing',True)
encoder=world_copy(body,'04 Encoder wall — original opening')
intersect_box(encoder,(-4,14.2,0),(22,18.5,12.5))
encoder=finish(encoder,'04_encoder_wall.stl','Original encoder opening: check shaft/bushing passage and wall clearance; does not validate the whole KY-040 module',True)

bpy.ops.mesh.primitive_cube_add(size=1)
dummy=bpy.context.object;dummy.name='05 Battery size gauge — 28x27x5.7'
for c in list(dummy.users_collection):c.objects.unlink(dummy)
collection.objects.link(dummy)
dummy.dimensions=(28,27,5.7)
bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
dummy=finish(dummy,'05_battery_dummy.stl','Solid nominal 28x27x5.7 battery substitute; excludes pouch leads/protection, tolerances and required clearances')

assert signature(sources)==original_signature,'Original geometry was changed'
# Keep original source geometry in its own scene; only the coupons are shown here.
preview_scene=bpy.data.scenes.new('PRINT CHECKS — original openings, no case modifications')
preview_scene.unit_settings.system='METRIC';preview_scene.unit_settings.scale_length=.001
preview_scene.unit_settings.length_unit='MILLIMETERS'
preview_scene.collection.children.link(collection);source_scene.collection.children.unlink(collection)
bpy.context.window.scene=preview_scene
for obj,center in zip([bezel,plate,ports,encoder,dummy],[(-34,20),(31,20),(-34,-18),(22,-18),(0,-48)]):
    size=[max(v.co[i] for v in obj.data.vertices) for i in range(3)]
    obj.location=(center[0]-size[0]/2,center[1]-size[1]/2,0)
    obj.color=(.22,.59,.51,1)

preview_scene['source_geometry_sha256']=original_signature
preview_scene['scope']='Fit coupons only. No verification of all components fitting together.'
text=bpy.data.texts.new('PRINT CHECKS — instructions')
text.write('''STL dimensions are millimetres. Slice at 100%, no automatic rescale.
01 original front cover/aperture; 02 original four-hole PCB gauge;
03 USB+SD wall spacing; 04 encoder opening; 05 battery size substitute.
Source geometry is retained unchanged in the original scene.
A passing wall check does not prove fit of the whole electronics assembly.
See print_checks/README.md and manifest.json for nominal dimensions.
''')
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            space=area.spaces.active;space.shading.type='SOLID';space.shading.color_type='OBJECT'
            space.shading.show_xray=False;space.overlay.show_extras=False
            space.region_3d.view_location=(0,-6,0);space.region_3d.view_distance=160
            space.region_3d.view_rotation=Vector((0,-75,145)).to_track_quat('Z','Y')
bpy.ops.object.select_all(action='DESELECT')
ports.select_set(True);bpy.context.view_layer.objects.active=ports
bpy.context.view_layer.update()
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'esp32s3_watch_box_print_checks.blend'))
(OUT/'manifest.json').write_text(json.dumps({'units':'mm','scale':1,'original_geometry_sha256':original_signature,
    'source_case_modified':False,'whole_assembly_fit_verified':False,'checks':checks},ensure_ascii=False,indent=2)+'\n')
# Render a tray preview; camera is not added to the saved fabrication scene.
bpy.ops.object.camera_add(location=(80,-120,170))
camera=bpy.context.object;camera.rotation_euler=(Vector((0,-6,0))-camera.location).to_track_quat('-Z','Y').to_euler()
camera.data.type='ORTHO';camera.data.ortho_scale=144;preview_scene.camera=camera
preview_scene.world=bpy.data.worlds.new('Fit tray preview world');preview_scene.world.color=(.12,.12,.12)
preview_scene.render.engine='BLENDER_WORKBENCH';preview_scene.display.shading.color_type='OBJECT'
preview_scene.display.shading.light='STUDIO';preview_scene.display.shading.show_shadows=True
preview_scene.render.resolution_x=1200;preview_scene.render.resolution_y=1000;preview_scene.render.resolution_percentage=100
preview_scene.render.image_settings.file_format='PNG';preview_scene.render.filepath=str(OUT/'preview.png')
bpy.ops.render.render(write_still=True)
print('FIT_COUPONS='+json.dumps(checks))
