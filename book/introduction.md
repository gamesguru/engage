# Introduction

It's very common to structure CI steps as a [directed acyclic graph (DAG)][dag],
and most CI platforms support this. Unfortunately, each one has a different way
to configure this DAG, and dealing with these is [often painful][pain]. Engage
provides a way to configure a DAG of CI steps separately from CI platforms that
can also be run locally.

[dag]: https://en.wikipedia.org/wiki/Directed_acyclic_graph
[pain]: https://blog.yossarian.net/2023/09/22/GitHub-Actions-could-be-so-much-better
