use crate::planning::classical::heuristics::*;
use crate::planning::classical::state::*;
use crate::planning::classical::search::*;
use crate::planning::classical::{GroundProblem};
use std::fmt::Display;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

//ajout pour gerer fichier
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};

//matrice facilite Dijktstra
use nalgebra::base::*;

//explication des liens entre 2 points (menaces, support...)
pub fn explicationsupport(plan: &Vec<Op>,support : &DMatrix<i32> , ground : &GroundProblem, step1: i32, step2:i32)->Necessaire{
	//dijkstra( plan, ground);
	let length=plan.len();
	let mut atraite=Vec::new();
	let mut traite=Vec::new();
	let l2=length as u32;

	/*Pour Chaque x∈S Faire δs(x)←∞  On attribue un poids ∞ à chacun des sommetsx
 δs(s0)←0   Le poids du sommet s0 est nul
 X←S    La liste des sommets restant à traiter est initialisée à S
 E←∅    La liste des sommets déjà traités vide
    */
    //cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2+1);
        //if mene à step1
		if count == step1 {
			nec = newnecgoal(step);
		}      
        atraite.push(nec);
        count=count+1;
    }

    /*traitement
Tant queX≠∅Faire            Tant que la liste des sommets restant à traiter n'est pas vide

    Sélectionner dans la liste X le sommet x avec δs(x) minimum
    Retirer le sommet x de la liste X
    Ajouter le sommet x à la liste E
    Pour Chaquey∈V+(x)∩XFaire    On examine tous les successeurs y du sommet x qui ne sont pas traités
        Siδs(y)>δs(x)+l(x,y)Alors
        δs(y)←δs(x)+l(x,y)      La distance du sommet s0 au sommet y est minimale
        p(y)←x             Le sommet x est le prédécesseur du sommet y
        Fin Si
    Fin pour

Fin Tant que
    */
    let mut done= false;
    while !done{
        //sommet chemin plus court
        let mut somme=atraite.get(0).unwrap().clone();
        let mut count = 0;
        let mut index=0;
        for i in &atraite{
            if i.long()<somme.long(){
                somme=i.clone();
                index=count;
            }
            count=count+1;
        }
        //println!{"     retire     {}",index};
        //
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //println!("essai entrée dijk");
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let ind=somme.opnec().numero() as usize;
            if support[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
					let b=i as i32;
					//println!("essai entrée dijk 01");
                    if res.opnec().numero()==b{
                        //println!("essai entrée dijk 1");
                        if res.long()>somme.long()+1{
                            //attention unwrap
                            let mut newchemin;
                            if somme.chemin().is_none(){
                                newchemin=Vec::new();
                            }else{
                                newchemin= somme.chemin().unwrap();
                            }   
                            newchemin.push(somme.opnec());
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+1);
                            newatraite.push(nec);
                        }
                        else{newatraite.push(res);}

                    }else{newatraite.push(res);}
                }
                atraite=newatraite.clone();
            }

        }
        if atraite.is_empty(){
            done=true;
        }
	}
	let s2=step2 as usize;
	let step =newresume(*plan.get(s2).unwrap(),step2);
	let mut nec =initnec(step,l2+1);
	for i in traite{
		if i.opnec().numero()==step2{
			nec= i;
		}
	}
	nec
}

pub fn explicationmenace(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , ground : &GroundProblem, step1: i32, step2:i32){
	//dijkstra( plan, ground);
	let length=plan.len();
    let l2=length as u32;
    let s1=step1 as usize;
    let s2=step2 as usize;
    let m=menace.get((s1,s2));
    if !m.is_none(){
        if *m.unwrap() == 0 {
            println!(" L'étape {} n'est pas une menace pour l'étape {}",step1, step2);
        }else if *m.unwrap() == 1{
            println!(" L'étape {} est une menace pour l'étape {} et doit être placer après",step1, step2);
        }else if *m.unwrap() == -1{
            println!(" L'étape {} est une menace pour l'étape {} et doit être placer avant",step1,step2);
        }else if *m.unwrap() == -2{
            //on regarde les support de s2
            let row=support.column(s2);
            for index in 0..l2+1{
                let i = index as usize;
                let support=row.get(i);
                if !support.is_none(){
                    if *support.unwrap()==1 {
                        //on regarde quel support est lié à la menace
                        let ms=menace.get((s1,i));
                        if !ms.is_none() {
                            if *ms.unwrap() == -1{
                                println!(" L'étape {} est une menace pour le lien entre l'étape {} et l'étape {}, l'étape {} doit être placer avant {}",step1, i,step2,step1, i);
                            }

                        }
                    }
                }
                
            }
            
        }else{
            println!("Erreur");
        }
    }



}

pub fn explication2etape(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , ground : &GroundProblem, step1: i32, step2:i32){
    println!("lien entre les étape {} et {}",step2,step1);
    if step1 > step2 {
        let nec=explicationsupport(plan, support, ground, step1, step2);
        nec.affiche();
    }else{
        let nec=explicationsupport(plan, support, ground, step2, step1);
        nec.affiche();
    }
    explicationmenace(plan, menace, support, ground, step1, step2);
    explicationmenace(plan, menace, support, ground, step2, step1);
}


pub fn choixpredicat() {}

pub fn dijkstrapoids(plan : &Vec<Op>,ground: &GroundProblem,mat : &DMatrix<i32>,predicat: &Vec<SVId>)->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    let infini=(l2*l2) as i32;

    //let dd=l2+1;
    let plan2=plan.clone();
    let plan3=plan.clone();
    let mut matrice=mat.clone();
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

    //
    // mettre les poids ici avec prédicats
    //
    let mut count=0;
    for a in 0..l2+1{
        for b in 0..l2+1{
            let i=a as usize;
            let j=b as usize;
            let m=matrice.get((i,j));
            if !m.is_none(){
                if *m.unwrap() == 1{
                    let support=plan.get(i);
                    let action=plan.get(j);
                    if !support.is_none() && !action.is_none(){
                        let s = *support.unwrap();
                        let a= *action.unwrap();
                        let precon = ops.preconditions(a);
                        let effet = ops.effects(s);
                        for pre in precon{
                            for eff in effet{
                                for p in predicat{
                                    if pre.var() == *p && eff.var()== *p{
                                            matrice[(i,j)]=infini;
                                    }
                                }
                                
                            }
                        }

                    }
                    
                }
            }
        }

    }

    //dijkstra

//pas touche
    let cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2+1);
        //if mene à goal
        for c in &cause{
            if c.numero()==count{
                nec = newnecgoal(step);
            }
        }        
        atraite.push(nec);
        count=count+1;
    }

    /*traitement
Tant queX≠∅Faire            Tant que la liste des sommets restant à traiter n'est pas vide

    Sélectionner dans la liste X le sommet x avec δs(x) minimum
    Retirer le sommet x de la liste X
    Ajouter le sommet x à la liste E
    Pour Chaquey∈V+(x)∩XFaire    On examine tous les successeurs y du sommet x qui ne sont pas traités
        Siδs(y)>δs(x)+l(x,y)Alors
        δs(y)←δs(x)+l(x,y)      La distance du sommet s0 au sommet y est minimale
        p(y)←x             Le sommet x est le prédécesseur du sommet y
        Fin Si
    Fin pour

Fin Tant que
    */
    let mut done= false;
    while !done{
        //sommet chemin plus court
        let mut somme=atraite.get(0).unwrap().clone();
        let mut count = 0;
        let mut index=0;
        for i in &atraite{
            if i.long()<somme.long(){
                somme=i.clone();
                index=count;
            }
            count=count+1;
        }
        //println!{"     retire     {}",index};
        //
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //println!("essai entrée dijk");
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let ind=somme.opnec().numero() as usize;
            //println!("essai entrée dijkstra {} {} {}",matrice[(i,ind)],i,somme.opnec().numero());
            if matrice[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
                    if res.opnec().numero()==(i as i32){
                        //println!("essai entrée dijk 1");
                        if res.long()>somme.long()+1{
                           /* 
                            
                            let chemi=chem.push(somme.opnec());
                            let chemmi=chemm.push(s);*/
                            //println!("essai entrée dijk 2");
                            //let chem=somme.chemin();
                            //attention unwrap
                            let mut newchemin;
                            if somme.chemin().is_none(){
                                newchemin=Vec::new();
                            }else{
                                newchemin= somme.chemin().unwrap();
                            }   
                            newchemin.push(somme.opnec());
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+1);
                            newatraite.push(nec);
                        }
                        else{newatraite.push(res);}

                    }else{newatraite.push(res);}
                }
                atraite=newatraite.clone();
            }

        }
        if atraite.is_empty(){
            done=true;
        }
    }
    traite
}
