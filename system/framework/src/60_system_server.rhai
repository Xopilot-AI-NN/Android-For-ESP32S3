fn system_server_health(){
    if service_state("package")!=1{return 0;}
    if service_state("activity")!=1{return 0;}
    if service_state("surfaceflinger")!=1{return 0;}
    if service_state("notification")!=1{return 0;}
    if service_state("media_session")!=1{return 0;}
    if service_state("connectivity")!=1{return 0;}
    if service_state("wearable")!=1{return 0;}
    if service_state("zpager")!=1{return 0;}
    1
}
