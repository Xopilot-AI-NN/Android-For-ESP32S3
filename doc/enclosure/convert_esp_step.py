from pathlib import Path
import json,sys
sys.path.append('/usr/lib/freecad/lib')
import FreeCAD,Part,MeshPart
root=Path(__file__).resolve().parent/'references'
s=Part.Shape();s.read(str(root/'esp32-s3-zero.stp'))
solids=[]
for i,q in enumerate(s.Solids):
 mesh=MeshPart.meshFromShape(Shape=q,LinearDeflection=.025,AngularDeflection=.15,Relative=False)
 points,faces=mesh.Topology
 b=q.BoundBox
 solids.append({'index':i,'vertices':[[p.x,p.y,p.z] for p in points],'faces':[list(f) for f in faces],'bounds':[b.XMin,b.XMax,b.YMin,b.YMax,b.ZMin,b.ZMax]})
(root/'esp32-s3-zero-mesh.json').write_text(json.dumps({'units':'mm','source':'Waveshare official ESP32-S3-Zero v2 STEP','linear_deflection_mm':.025,'solids':solids},separators=(',',':')))
print('SOLIDS',len(solids),'TRIANGLES',sum(len(s['faces']) for s in solids))
