# symapper

A ranked map of the code that matters for your task, within a token budget.

[![npm](https://img.shields.io/npm/v/symapper)](https://www.npmjs.com/package/symapper)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://github.com/viewtools-app/symapper/blob/main/LICENSE)

symapper is a local CLI that prints signatures of the most relevant definitions and the files
that use them. No LLM, no daemon, no network while mapping. Node.js 18 or later.

```sh
npx symapper .                              # map the current repo
npx symapper . --focus cache --tokens 800   # focus a word or path, set a budget
npm i -g symapper                           # or install the command globally
```

Platforms: macOS arm64 and x64, Linux x64 and arm64 (static musl). The binary comes from an
optional platform package, so keep optional dependencies enabled. Windows is not supported.

[Website](https://symapper.viewtools.app/) |
[Docs](https://symapper.viewtools.app/docs.html) |
[GitHub](https://github.com/viewtools-app/symapper)

License: Apache-2.0.
