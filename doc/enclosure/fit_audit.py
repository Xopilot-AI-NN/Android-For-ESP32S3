"""Conservative mesh intersection diagnostics, not a manufacturing certificate.

Tests cross-component surfaces plus containment. A mesh collision is a blocker;
no collision does not establish tolerance, connector access or missing geometry.
"""
from mathutils import Vector
from mathutils.bvhtree import BVHTree

EPS=.02

def prepared(obj,deps):
    evaluated=obj.evaluated_get(deps);mesh=evaluated.to_mesh()
    points=[obj.matrix_world@v.co for v in mesh.vertices]
    faces=[tuple(p.vertices) for p in mesh.polygons]
    bounds=[(min(v[i] for v in points),max(v[i] for v in points)) for i in range(3)]
    bvh=BVHTree.FromPolygons(points,faces,epsilon=0)
    evaluated.to_mesh_clear()
    return {'obj':obj,'points':points,'bounds':bounds,'bvh':bvh}

def inside(point,shape):
    # Oblique ray avoids the common coplanar grid edges of axis-aligned models.
    direction=Vector((.813,.451,.369)).normalized()
    origin=point.copy();count=0
    for _ in range(128):
        hit,normal,index,distance=shape['bvh'].ray_cast(origin,direction,10000)
        if hit is None:break
        if distance<EPS:return False  # A touching point is not evidence of penetration.
        count+=1;origin=hit+direction*EPS
    return count%2==1

def intersects(a,b):
    overlap=[min(x[1],y[1])-max(x[0],y[0]) for x,y in zip(a['bounds'],b['bounds'])]
    if min(overlap)<=EPS:return None
    surface=bool(a['bvh'].overlap(b['bvh']))
    contained=False
    if not surface:
        # Coplanar box faces can evade triangle intersection, and all boundary
        # vertices can lie on the other solid's faces. Probe the overlap interior.
        ranges=[(max(x[0],y[0]),min(x[1],y[1])) for x,y in zip(a['bounds'],b['bounds'])]
        fractions=(.25,.5,.75)
        for fx in fractions:
            for fy in fractions:
                for fz in fractions:
                    point=Vector([lo+(hi-lo)*f for (lo,hi),f in zip(ranges,(fx,fy,fz))])
                    if inside(point,a) and inside(point,b):
                        return {'overlap_bounds_mm':[round(v,3) for v in overlap], 'overlap_box':[[lo,hi] for lo,hi in ranges], 'evidence':'sampled_overlap_interior'}
        contained=any(inside(p,b) for p in a['points'][::max(1,len(a['points'])//32)]) or any(inside(p,a) for p in b['points'][::max(1,len(b['points'])//32)])
    return {'overlap_bounds_mm':[round(v,3) for v in overlap], 'overlap_box':[[max(x[0],y[0]),min(x[1],y[1])] for x,y in zip(a['bounds'],b['bounds'])], 'evidence':'surface_intersection' if surface else 'sampled_containment'} if surface or contained else None

def audit(scene,assembly,original):
    deps=__import__('bpy').context.evaluated_depsgraph_get()
    roots=[o for o in assembly.objects if o.type=='EMPTY']
    shapes={root.name:[prepared(o,deps) for o in root.children if o.type=='MESH' and not o.get('visual_only',False)] for root in roots}
    collisions=[];groups=set()
    for i,a in enumerate(roots):
        for b in roots[i+1:]:
            found=[]
            for x in shapes[a.name]:
                for y in shapes[b.name]:
                    result=intersects(x,y)
                    if result:found.append({'a_part':x['obj'].name,'b_part':y['obj'].name,**result})
            if found:
                collisions.append({'a':a.name,'b':b.name,'intersections':found});groups.update([a.name,b.name])
    body=next(o for o in original if o.name.startswith('CASE —'))
    shell=prepared(body,deps);shell_collisions=[]
    for root in roots:
        found=[]
        for child in shapes[root.name]:
            result=intersects(child,shell)
            if result:found.append({'part':child['obj'].name,**result})
        if found:shell_collisions.append({'component':root.name,'intersections':found});groups.add(root.name)
    return {'method':'mesh surface intersections + sampled containment; 0.02 mm contact threshold',
            'limits':'Conservative blockers, not exact overlap volumes. Missing components, model identity, clearances and tolerances are not verified.',
            'component_collisions':collisions,'case_collisions':shell_collisions,
            'collision_groups':sorted(groups),'component_pair_count':len(collisions),
            'case_collision_group_count':len(shell_collisions),
            'all_component_bounds':{root.name:[[round(min(s['bounds'][i][0] for s in shapes[root.name]),3),round(max(s['bounds'][i][1] for s in shapes[root.name]),3)] for i in range(3)] for root in roots}}
