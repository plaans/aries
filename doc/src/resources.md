# Resources

A resource $R(p_1,\dots,p_n)$ in Aries is represented by an **integer state variable**.
It has a **bounded domain** $[l,u]$ which restrict the allowed values taken by the resource.

For easier reading, we note $R^k$ the resource $R(p^k_1,\dots,p^k_n)$.

## 1. Manipulation

### 1.1. Conditions

A resource can be tested in conditions in order to check its current value.
The supported tests are: `<`, `<=`, `>`, `>=`, `==`, `!=`.
The right-hand side of the operator could be either a constant or a variable.

### 1.2 Effects

The value of a resource can be :

- changed with an **assignment**, the new value could be either a constant or a variable
- **increased** by a constant value
- **decreased** by a constant value, internally it is an increased operation with a negative constant value

## 2. Linear constraint representation

### 2.1. Effects

Every time a resource value is updated, new constraints or conditions are created in order to check that the value can be updated and that the new value is contained in the bounded domain of the resource.

#### 2.1.1 Assign effects

For an assign effect $[t^a] R^a := z$, we will create the following constraints:

$$
\begin{cases}
[t^a] z \ge l \\
[t^a] z \le u \\
\end{cases}
$$

#### 2.1.2 Increase effects

For an increase effect $[t^i] R^i \mathrel{+}= c$, we will create a new variable $v$ with the same domain as the resource and add a new condition $[t^i]R^i = v$.

This will ensure that the new value of the resource is in the correct domain after the increase.
Next, this condition is converted into linear constraints as explained in the next section.

#### 2.1.3 Decrease effects

For a decrease effect $[t^d] R^d \mathrel{-}= c$, we convert it into the increase effect $[t^d] R^d \mathrel{+}= (-c)$ and then create the associated conditions.

### 2.2. Conditions

Every time a resource must be tested, a set of associated linear constraints are created.

#### 2.2.1 Equality conditions

For an equality condition $[t^c] R^c = z$, we will have the **linear constraints**

$$
\begin{cases}
l^a_1c^a_1 + l^i_{11}c^i_1 + \dots + l^i_{1n}c^i_n - l^a_1z = 0 \\
\vdots \\
l^a_kc^a_k + l^i_{k1}c^i_1 + \dots + l^i_{kn}c^i_n - l^a_kz = 0 \\
\end{cases}
$$

Where $l^a_jc^a_j$ represents an assignment to the resource and $l^i_{jm}c^i_m$ represents an increase, the $l$ variables are boolean literals used to know if the associated $c$ variable (which is the value of the effect) should be used.
The idea is to consider only the constraint corresponding to the latest assignment.

To do so, we define $\textit{SP}$ which test if the parameters of two resources are the same by:

$$\textit{SP}(x,y) \Leftrightarrow \bigwedge_{b} (p^x_b=p^y_b)$$

We also define $\textit{LA}$ which test if the assignment effect is the last one before the condition.
**There are two ways to define it**.
The **first** one is to check that the effect is before the condition and for other compatible assignment effects, check that they are not between the considering effect and the condition:

$$\textit{LA}(j) \Leftrightarrow t^a_j \le t_c \land \bigwedge_{b \ne j} (\neg \textit{prez}_b \lor \neg\textit{SP}(b,c) \lor t^a_b < t^a_j \lor t^a_b > t^c)$$

The **second** way to define $\textit{LA}$ is to add a fourth time point to the numeric assignment effects.
Currently, the effects have three time points: the start of the transition, the start of the persistence, and the end of the persistence.
We add a fourth one, the **end of the assignment**, which holds while the current assignment is the last one.
This time point is forced to be after or equals to the end of the persistence.
This way, we just need to check that the condition is between the start of the persistence $t^a_{j,s}$ and the end of the assignment $t^a_{j,e}$:

$$\textit{LA}(j) \Leftrightarrow t^a_{j,s} \le t_c \land t_c \le t^a_{j,e}$$

At the end, the $l^a_j$ literal is true if the effect is present, is the effect before the condition, and has the same resource as the condition:

$$l^a_j \Leftrightarrow \textit{prez}_j \land \textit{LA}(j) \land \textit{SP}(j,c)$$

Moreover, we enforce to have at least one last assignment effect with the disjunction $\bigvee_{j}l^a_j$.

The $l^i_{jm}$ literal is true if the increase $m$ is present with the same resource as the condition, if the assignment $j$ is the last one and if the increase $m$ is between the assignment $j$ and the condition:

$$l^i_{jm} \Leftrightarrow \textit{prez}_m \land \textit{SP}(m,c) \land l^a_j \land t^a_j < t^i_m \land t^i_m \le t_c$$

#### 2.2.2 Others conditions

For others conditions, we convert them into equality conditions.
For example, the condition $[t^c] R^c \le z$ is converted into

$$
\begin{cases}
[t^c] R^c = z' \\
[t^c] z' \le z
\end{cases}
$$
