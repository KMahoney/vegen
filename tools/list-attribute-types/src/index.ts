import * as ts from "typescript";

function resolveLibDom(): string {
  try {
    // Resolve the lib.dom.d.ts that comes with the installed typescript package
    // This will work after typescript is installed in the project
    // Using require.resolve ensures we find the file inside the node_modules/typescript package.
    // If not found, fall back to common locations (not expected here).
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const resolved = require.resolve("typescript/lib/lib.dom.d.ts");
    return resolved;
  } catch (err) {
    throw new Error(
      "Could not resolve 'typescript/lib/lib.dom.d.ts'. Make sure `typescript` is installed."
    );
  }
}

function collectAndResolve(libPath: string): Record<
  string,
  {
    elementType: string;
    attributes: Record<string, string>;
  }
> {
  const program = ts.createProgram([libPath], {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.CommonJS,
    lib: [],
    allowJs: false,
  });
  const checker = program.getTypeChecker();
  const sf = program.getSourceFile(libPath);
  if (!sf) {
    throw new Error(`Failed to load source file: ${libPath}`);
  }

  const result: Record<
    string,
    {
      elementType: string;
      attributes: Record<string, string>;
    }
  > = {};

  function visit(node: ts.Node) {
    if (
      ts.isInterfaceDeclaration(node) &&
      node.name &&
      node.name.text === "HTMLElementTagNameMap"
    ) {
      for (const member of node.members) {
        if (ts.isPropertySignature(member) && member.name) {
          // Resolve tag name (could be identifier or string literal)
          const nameNode = member.name;
          let tagName: string;
          if (ts.isIdentifier(nameNode)) {
            tagName = nameNode.escapedText.toString();
          } else if (
            ts.isStringLiteral(nameNode) ||
            ts.isNumericLiteral(nameNode)
          ) {
            tagName = nameNode.text;
          } else {
            tagName = nameNode.getText();
          }

          // Resolve the element type for this property
          let elementType: ts.Type | undefined;
          if (member.type) {
            try {
              elementType = checker.getTypeFromTypeNode(member.type);
            } catch {
              // fallback
              elementType = checker.getTypeAtLocation(member);
            }
          } else {
            elementType = checker.getTypeAtLocation(member);
          }

          const elementTypeStr = elementType
            ? checker.typeToString(elementType)
            : "unknown";

          const attributes: Record<string, string> = {};

          if (elementType) {
            // Get the apparent type to include inherited properties
            const apparent = checker.getApparentType(elementType);
            const props = checker.getPropertiesOfType(apparent);

            for (const p of props) {
              // Use the first declaration as the location for type resolution
              const decl =
                (p.valueDeclaration as ts.Declaration) ||
                (p.declarations && p.declarations[0]);
              if (!decl) continue;

              // Skip methods (we only want attribute-like properties)
              // Method signatures/declarations produce callable types; skip them
              if (
                ts.isMethodSignature(decl) ||
                ts.isMethodDeclaration(decl) ||
                ts.isFunctionDeclaration(decl)
              ) {
                continue;
              }

              // Some properties are index signatures or symbol-based; ensure we have a name
              const propName = p.getName();
              if (!propName) continue;

              // Get the type of the property at the declaration location
              let propType: ts.Type;
              try {
                propType = checker.getTypeOfSymbolAtLocation(p, decl);
              } catch {
                // fallback to any
                propType = checker.getAnyType();
              }

              const propTypeStr = checker.typeToString(propType);
              attributes[propName] = propTypeStr;
            }
          }

          result[tagName] = {
            elementType: elementTypeStr,
            attributes,
          };
        }
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(sf);
  return result;
}

function main() {
  try {
    const libPath = resolveLibDom();
    const resolved = collectAndResolve(libPath);

    // Print nested JSON of tag -> attribute -> type
    const out: Record<string, Record<string, string>> = {};

    for (const tag of Object.keys(resolved).sort()) {
      const info = resolved[tag];
      const attrNames = Object.keys(info.attributes).sort();
      const attrs: Record<string, string> = {};
      for (const a of attrNames) {
        const rawType = info.attributes[a] ?? "unknown";
        attrs[a] = rawType;
      }
      out[tag] = attrs;
    }

    console.log(JSON.stringify(out, null, 2));
  } catch (err: any) {
    console.error("Error:", err.message || err);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}
