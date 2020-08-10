//use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::symbols::{SymbolTable,SymId};
use crate::classical::{GroundProblem};
use crate::explain::state2::*;
use crate::explain::explain::*;
use std::fmt::Display;
use nalgebra::base::*;
use std::collections::HashMap;

pub fn calculcentraliteglobal(support : &DMatrix<i32>)->Vec<f32>{
    
    let mut out=Vec::new();
    let i: usize = support.nrows();
    let j: usize = support.ncols();
   // vec![0; i];
    let mut sumligne=vec![0;i];
    let mut sumcolonne=vec![0; j];
    for row in 0..i{
        for col in 0..j{
            if support[(row,col)]!=0{
                sumligne[row]=sumligne[row]+1;
                sumcolonne[col]=sumcolonne[col]+1;
            }
        }
    }
    for index in 0..i{
        let int=sumcolonne[index] as f32;
        let ou=sumligne[index] as f32;
        //println!("{} {}",int,ou);
        out.push(int/ou);
    }
    out
}

pub fn calculcentraliteglobal2(support : &DMatrix<i32>)->Vec<(usize,usize)>{
    
    let mut out=Vec::new();
    let i: usize = support.nrows();
    let j: usize = support.ncols();
   // vec![0; i];
    let mut sumligne=vec![0;i];
    let mut sumcolonne=vec![0; j];
    for row in 0..i{
        for col in 0..j{
            if support[(row,col)]!=0{
                sumligne[row]=sumligne[row]+1;
                sumcolonne[col]=sumcolonne[col]+1;
            }
        }
    }
    for index in 0..i{
        /*let int=sumcolonne[index] as f32;
        let ou=sumligne[index] as f32;
        //println!("{} {}",int,ou);*/
        out.push((sumcolonne[index],sumligne[index]));
    }
    out
}

pub fn regroupementcentralite (centra: &Vec<(usize,usize)>,plan: &Vec<Op>)->HashMap<(usize,usize),Vec<Resume>>{
    let taille=centra.len();
    let mut regroup = HashMap::new();
    //Hmap
    for i in 0..taille{
        if !plan.get(i).is_none(){
            let index = i as i32;
            //crea du resume de l'étape
            let r=newresume(*plan.get(i).unwrap(), index);
            let mut key = *centra.get(i).unwrap();
            //ajout condition if (n,n)->(1,1)
            let (a,b)= *centra.get(i).unwrap();
            if a==b{
                //print!("{:?}",key);
                key= (1,1);
                //println!("chngmt key")
            }
            let essai= regroup.get_mut(&key);
            //println!("{:?}",key);
            if essai.is_none(){
                let mut v=Vec::new();
                v.push(r);
                regroup.insert(key,v);
                //println!("====----------       -----     --------=== C5 pas normal {:?}", key);
            }else{
                let v=essai.unwrap();
                v.push(r);
                //println!("======= C6 {:?}", key);
            }
        }
    }
    regroup
}



pub fn regroupementcentraliteop(centra: &Vec<f32>,plan: &Vec<Op>)->HashMap<Op,Vec<f32>>{
    let taille=centra.len();
    let mut regroup = HashMap::new();
    for i in 0..taille{
        if !plan.get(i).is_none(){
            if regroup.get_mut(plan.get(i).unwrap()).is_none(){
                let mut  v=Vec::new();
                v.push(*centra.get(i).unwrap());
                regroup.insert(*plan.get(i).unwrap(),v);
            }else{
                let essai= regroup.get_mut(plan.get(i).unwrap()).unwrap();
                essai.push(*centra.get(i).unwrap());
            }
        }
    }
    regroup
}

pub fn regroupementcentraliteaction (centra: &Vec<f32>,plan: &Vec<Op>, ground: &GroundProblem, symbol : &SymbolTable<String,String>)->HashMap<SymId,Vec<f32>>{
    let taille=centra.len();
    let mut regroupe = HashMap::new();

    //Compter nombre D'action (SymId même principe que Op)
    let mut nbop=0;
    let mut v=Vec::new();
    for i in plan{
        if v.is_empty(){
            let action = ground.operators.name(*i)[0];
            v.push(action);
            let mut vec=Vec::new();
            regroupe.insert(action,vec);
            nbop=nbop+1;
        }else{
            let mut notin=true;
            for ope in &v{
                let action=&ground.operators.name(*i);
                if *ope == action[0] {
                    
                    notin=false;
                }
            }
            if notin {
                let action = ground.operators.name(*i)[0];
                v.push(action);
                let mut vec=Vec::new();
                regroupe.insert(action,vec);
                nbop=nbop+1;   
            }
        }
    }

    for index in 0..taille{
        if !plan.get(index).is_none(){
            let action =ground.operators.name(*plan.get(index).unwrap())[0];
            if regroupe.get_mut(&action).is_none(){
                let mut  v=Vec::new();
                v.push(*centra.get(index).unwrap());
                regroupe.insert(action,v);
            }else{
                let essai= regroupe.get_mut(&action).unwrap();
                essai.push(*centra.get(index).unwrap());
            }
        }
    }
    
    regroupe
}

pub fn affichagehmapop<T,I : Display>(val:HashMap<Op,Vec<f32>>,ground: &GroundProblem,symbol: &World<T,I> ){
    for (i,v) in val.iter(){
        print!("L'opérateur {} numéroté ",symbol.table.format(&ground.operators.name(*i)));
        println!("{:?} de centralité : ",*i);
        
        for n in v{
            print!("{}, ", *n);
        }
        println!("");
    }
}

pub fn affichagehmapaction<T,I : Display>(val:HashMap<SymId,Vec<f32>>,symbol: &World<T,I> ){
    for (i,v) in val.iter(){
        let vecinter = vec![*i];
        let slice = &vecinter[..];
        println!("L'action {} de centralité :",symbol.table.format(slice));        
        for n in v{
            print!("{}, ", *n);
        }
        println!("");
    }
}

pub fn affichageregroucentra<T,I : Display>(val:HashMap<(usize,usize),Vec<Resume>>,ground: &GroundProblem,symbol: &World<T,I> ){
    println!("======= SUUUUU {}",val.len());
    for i in val.keys(){
        println!("======= centralite {:?}",i);
        for d in val.get(&i){
            for r in d{
                print!("L'opérateur {:?} de l'étape {} alias ",r.op(),r.numero());
                println!("L'opérateur {} de l'étape {}",symbol.table.format(&ground.operators.name(r.op().unwrap())),r.numero());
            }
            
        }
    }
}

/*
//algo naif adapté à notre situation
pub fn centraliteintermediarite(support : &DMatrix<i32>){
    let taille = support.len();
    //visite de tous les noeuds pour avoir leur centralité d'intermédiarité
    for i in 0..taille {
        //visite des noeuds précédents
        for prec in 0..i{
            //visites des noeuds suivants
            for suiv in i..taille{
                intermediarite(i,prec,suiv,support);
            }
        }
    }
}

//bfs
pub fn intermediarite (etape: usize,precedent:usize,suivant: usize,support : &DMatrix<i32>)->Option<Vec<usize>>{
    let taille= support.len();
    let mut out = None;
    let mut queue = Vec::new();
    let mut discovered = Vec::new();
    let mut listechemin=Vec::new();//transfo en Hashmap? usize vec<vec<
    queue.push(precedent);
    if precedent>etape || suivant<etape{
        out
    }
    else{
        let step = queue.remove(0);
        discovered.push(step);
        for i in step..taille-1{
            if suivant<step{
                //out
            }
            else if support[(step,i)]==1{
                queue.push(i);
                discovered[i]=discovered[step]+1;
            }

        }
        if {

        }else if


        out
    }
}
/*
pub fn allpath(paths: &Vec<Vec<usize>>,chemin: &Vec<usize>,parent:Vec<usize>,a: usize,b:usize){
    if u ==1{
        paths.push(chemin);
    }else{
        //tous les parents de u
        for par in parent{
            if parent
        }
    }
}*/

pub fn bfs(start : usize,stop : usize,support : &DMatrix<i32>){
    let mut queue= Vec::new();
    let taille = support.len();
    let mut visited= Vec::with_capacity(taille-1);
    let mut parents=Vec::with_capacity(taille-2);
    while !queue.is_empty(){
        let node = queue.remove(0);
        if node == end{
            //stop

        }
        for i in 0..taille{
            if support[(node,i)]==1{
                if visited[i]==0{
                    visited[i]=visited[node]+1
                    queue.push(i);
                    parent[i].push(node);
                }
                else if visited[node]=visited[i]-1{
                    parent[i].push(node);
                }
            }
        }

    }
    //retour...
}
*/


/*
//algo brandes
pub fn brandes(support : &DMatrix<i32>){
    let taille = support.len();
    let mut centrainter=Vec::with_capacity(taille-1);
    //visite de tous les noeuds
    for i in 0..taille-1{
        let mut pile =Vec::new();
        let P
    }

}*/

//betweeness centrality 

pub fn betweeness (support : &DMatrix<i32>)->Vec<f32>{
    let taille = support.nrows();
    let mut cb = Vec::with_capacity(taille-1);

    
    for sommet in 0..taille-1{
        //INIT
        let mut stack = Vec::new();
        //let mut parents=Vec::with_capacity(taille-1);
        let mut parents=vec![Vec::new();taille-1];
        //let mut sigma=Vec::with_capacity(taille-1);
        let mut sigma=vec![0;taille-1];
        sigma[sommet]=1;
        //let mut dist=Vec::with_capacity(taille-1);
        let mut dist=vec![-1;taille-1];
        dist[sommet]=0;
        let mut q=Vec::new();
        q.push(sommet);
        //calcul sigma = nb plus court chemin de w
        while !q.is_empty() {
            let node=q.remove(0);
            stack.push(node);
            for voisin in 0..taille-1{
                if support[(node,voisin)]==1{
                    //si premier parcours du voisins
                    if dist[voisin]<0{
                        q.push(voisin);
                        dist[voisin]=dist[node]+1;
                    }
                    //plus court chemin de voisins via v?
                    if dist[voisin]==dist[node]+1{
                        sigma[voisin]==sigma[voisin]+sigma[node];
                        parents[voisin].push(node);
                    }
                }
            }
        }
        //calcul intermédiarité par rapport au sommet
        /*let mut delta=Vec::with_capacity(taille-1);
        delta.fill(0.0);*/
        let mut delta = vec![0.0;taille-1];
        while !stack.is_empty(){
            let dernier= stack.pop().unwrap();
            for i in &parents[dernier]{
                let s1 = sigma[*i] as f32;
                let s2 = sigma[dernier] as f32;
                delta[*i]=delta[*i]+s1/s2*(1.+delta[dernier]);
            }
            if dernier != sommet {
                cb[dernier]=cb[dernier]+delta[dernier];
            }
        }
    }
    cb
}

//Floyd Warshall
//tous les plus court chemins
pub fn floydwarshall(support : &DMatrix<i32>)->DMatrix<i32>{
    let taille = support.nrows();
    let t = taille as i32;
    let taille1=taille-1;
    let t1=t-1;
    let mut dist= DMatrix::from_diagonal_element(taille1,taille1,0);
    let td=dist.nrows();
    for l in 0..td{
        for c in 0..td{
            if support[(l,c)]==1  {
                dist[(l,c)]=1;
            }
            else if l!=c{
                dist[(l,c)]=t
            }
        }
    }
    for k in 0..taille-2{
        for l in 0..taille-2{
            for c in l..taille-2{
                if dist[(l,c)]>dist[(l,k)]+dist[(k,c)]{
                    dist[(l,c)]=dist[(l,k)]+dist[(k,c)];
                }
            }
        }
    }
    dist
}

//floy warshall shortest path
pub fn floydwarshallpath(support : &DMatrix<i32>)->(DMatrix<i32>,DMatrix<i32>){
    let taille = support.nrows();
    let t = taille as i32;
    let taille1=taille-1;
    let t1=t-1;
    let mut dist= DMatrix::from_diagonal_element(taille1,taille1,0);
    let mut next= DMatrix::from_diagonal_element(taille1,taille1,0);
    let td=dist.nrows();
    for l in 0..td{
        for c in 0..td{
            if support[(l,c)]==1  {
                dist[(l,c)]=1;
                next[(l,c)]=c as i32;
            }
            else if l!=c{
                dist[(l,c)]=t;
                next[(l,c)]=-1;

            }
            else if l==c{
                next[(l,c)]=c as i32;
            }
        }
    }
    for k in 0..taille-2{
        for l in 0..taille-2{
            for c in l..taille-2{
                if dist[(l,c)]>dist[(l,k)]+dist[(k,c)]{
                    dist[(l,c)]=dist[(l,k)]+dist[(k,c)];
                    next[(l,c)]=next[(l,k)];
                }
            }
        }
    }
    (dist,next)
}

pub fn path(u: usize, v: usize, next: &DMatrix<i32>)->Vec<i32>{
    let mut out=Vec::new();
    if next[(u,v)]==-1{
        return out;
    }
    let mut var =u as i32;
    out.push(var);
    while u!=v{
        let var2=var as usize;
        var=next[(var2,v)];
        out.push(var);
    }
    return out;
}/*

//faire sélection des chemin entre a et b contenant c
pub fn centrainter2(etape : usize,start: usize,end : usize,pcc : &DMatrix<i32>)->(i32,i32){
    let taille = pcc.nrows();
    let mut cietape = 0;
    let mut ci = 0;
    if pcc[(start,end)]< taille{
        ci= 1;
    }else{
        (cietape,ci)
    }
    if /*pcc[(start,etape)]>= taille || pcc[(etape,end)]>= taille ||*/ pcc[(etape,end)]+pcc[(start,etape)]>pcc[(start,end)] {
        (cietape,ci)
    }else{
        ci=0
        for i in 0..taille{
            if pcc[(i,end)]<taille{
                ci=ci+1;
            }

        }

    }
}*/