fn system_server_health(){
    if service_state("package")!=1{return 0;}
    if service_state("activity")!=1{return 0;}
    if service_state("connectivity")!=1{return 0;}
    if service_state("zpager")!=1{return 0;}
    1
}
