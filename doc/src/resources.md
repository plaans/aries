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

Every time a resource value is updated, new conditions are created in order to check that the value can be updated and that the new value is contained in the bounded domain of the resource.

For an **assign effect** $[t^a] R^a := z$, we will create the following conditions:
$$\begin{cases}
[t^a] z \ge l \\
[t^a] z \le u \\
\end{cases}$$

For an **increase effect** $[t^i] R^i \mathrel{+}= c$, we will create the following conditions:
$$\begin{cases}
[t^i] R^i \le u - c \\
[t^i] R^i \ge l - c \\
[t^i+\varepsilon] R^i \le u \\
[t^i+\varepsilon] R^i \ge l \\
\end{cases}$$

For a **decrease effect** $[t^d] R^d \mathrel{-}= c$, we convert it into the increase effect $[t^d] R^d \mathrel{+}= (-c)$ and then create the associated conditions.

Next, these conditions are converted into linear constraints as explained in the next section.

### 2.2. Conditions

Every time a resource must be tested, a set of associated linear constraints are created.

For an **equality condition** $[t^c] R^c = z$, we will have the **linear constraints**
$$\begin{cases}
l^a_1c^a_1 + l^i_{11}c^i_1 + \dots + l^i_{1m}c^i_m - z \le 0 \\
\dots \\
l^a_qc^a_q + l^i_{q1}c^i_1 + \dots + l^i_{qm}c^i_m - z \le 0 \\
\end{cases}$$

Where $l^a_jc^a_j$ represents an assignment to the resource and $l^i_{jk}c^i_k$ represents an increase, the $l$ variables are booleans used to know if the associated $c$ variable (which is the value of the assignment or the increase) should be used. The idea is to consider only the constraint corresponding to the latest assignment. 

To do so, we define $\textit{SP}$ which test if the parameters of two resources are the same, $\textit{SPP}$ which test the presence and the parameters, $\textit{LA}$ which test if the assignment is the last one for the current condition:
$$\begin{cases}
\textit{SP}(R^1,R^2) &\Leftrightarrow \bigwedge_{b} (p^1_b=p^2_b) \\
\textit{SPP}(j, b) &\Leftrightarrow \textit{SP}(R^j,R^b) \land \textit{prez}(j) \land \textit{prez}(b) \\
\textit{LA}(j) &\Leftrightarrow \bigwedge_{b \ne j} (t^a_b \le t^c \land \textit{SPP}(j,b) \implies t^a_b < t^a_j) \\
\end{cases}$$

Using these intermediate variables, we obtain
$$\begin{cases}
\bigvee_{b} l^a_b \\
l^a_j \Leftrightarrow t^a_j \le t^c \land \textit{LA}(j) \land \textit{SPP}(j,c) \\
l^i_{jk} \Leftrightarrow l^a_j \land t^a_j < t^i_k \land t^i_k < t^c \land \textit{SPP}(k,c) \\
\end{cases}$$

For **others conditions**, we convert them into equality conditions. For example, the condition $[t^c] R^c \le z$ is converted into
$$\begin{cases}
[t^c] R^c = z' \\
[t^c] z' \le z
\end{cases}$$