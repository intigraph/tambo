# quipu stack — memory your agents have to earn.

*Cords are entities. Knots are facts. The keeper decides what a knot may claim.*

Memory your coding agents have to earn: a local-first knowledge graph that
refuses bad facts at write time, plus the code context, crew harness, and
installer around it.

[Quipu](https://scbrown.github.io/quipu/) is the store: a governed, bitemporal
knowledge graph. Camayoc prepares knowledge for it, Bobbin returns knowledge
and repository context to the next agent turn, and Yupana contributes live
code structure. **Caboodle is the installer**: install the tools, prove they
work, and prepare their observability configuration.

![The quipu stack loop: agents send activity to Camayoc, which sends tagged, quarantined episodes to Quipu. Quipu supplies knowledge to Bobbin, which feeds the next agent turn. Yupana contributes committed structure to Quipu and code context to Bobbin. Caboodle installs, proves, and observes the tools.](images/stack-loop.svg)

## Three ways in

### 1. Try it in your browser

**[Open Quipu Explore — no installation](https://scbrown.github.io/quipu/explore/).**
The real store runs as WebAssembly in your browser. Use the SPARQL box, edit
a fact, and export a knowledge pack.

### 2. Start in Claude Code

Add the Camayoc plugin, then run its bootstrap:

```text
/plugin marketplace add scbrown/camayoc
/plugin install camayoc@camayoc
/camayoc:bootstrap
```

Camayoc brings its own Quipu, loads the ontology, and proves the write gate
with probes that must be refused. See the
[Camayoc setup guide](https://github.com/scbrown/camayoc#install).

### 3. Install the whole kit

[Install Caboodle](installing.md), then start its guided interview:

```bash
caboodle init --guided
```

Review the generated plan before applying it. The
[getting-started guide](getting-started.md) walks through installation,
verification, and recovery; [profiles](profiles.md) let you choose how much
of the stack to install.

## Around the stack

Explore [Skein](https://github.com/scbrown/skein),
[Shuttle](https://github.com/scbrown/shuttle),
[Creel](https://github.com/scbrown/creel),
[Shanty](https://github.com/scbrown/shanty),
[Desire Path](https://github.com/scbrown/desire-path), and
[Yupana on its own](https://github.com/scbrown/yupana).
Agents can run under [Shantytown](https://github.com/scbrown/shantytown) or Creel.

Work-tracking dependencies include the
[beads_rust fork](https://github.com/scbrown/beads_rust), based on
[Dicklesworthstone/beads_rust](https://github.com/Dicklesworthstone/beads_rust).
See [the stack](stack.md) for the component and dependency list.
