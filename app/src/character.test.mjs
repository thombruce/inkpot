// Run: node app/src/character.test.mjs
import assert from "node:assert/strict";
import { scaffoldCharacter } from "./character.js";

// Into an existing section: the entry lands under the heading, name selected.
{
  const src = "% Characters\n\n%% Alice\n";
  const { text, selFrom, selTo } = scaffoldCharacter(src);
  assert.ok(text.includes("%% New Character\nrole: "), `entry missing:\n${text}`);
  assert.ok(text.includes("%% Alice"), "existing character clobbered");
  assert.equal(text.slice(selFrom, selTo), "New Character", "selection off the name");
  // The heading is not duplicated.
  assert.equal(text.match(/^% Characters$/gm).length, 1, "section duplicated");
}

// No section yet: one is created, separated from prior content by a blank line.
{
  const src = "# Chapter\n\nProse.\n";
  const { text, selFrom, selTo } = scaffoldCharacter(src);
  assert.ok(text.includes("\n\n% Characters\n\n%% New Character"), `section not created:\n${text}`);
  assert.equal(text.slice(selFrom, selTo), "New Character");
}

// Empty document: no leading blank padding.
{
  const { text } = scaffoldCharacter("");
  assert.ok(text.startsWith("% Characters\n\n%% New Character"), `bad empty scaffold:\n${text}`);
}

// A custom name is used and selected.
{
  const { text, selFrom, selTo } = scaffoldCharacter("% Characters\n", "Bob");
  assert.ok(text.includes("%% Bob\n"), "custom name missing");
  assert.equal(text.slice(selFrom, selTo), "Bob");
}

// A deeper `%% Characters` section is matched, and the entry nests one marker
// deeper (`%%%`) — not a stray top-level `% Characters` (the manual-test bug).
{
  const src = "# Book\n\n## Chapter 1\n\n%% Characters\n\n%%% Alice\n";
  const { text } = scaffoldCharacter(src);
  assert.ok(text.includes("%%% New Character\nrole: "), `not nested under %%: \n${text}`);
  assert.ok(!/^% Characters$/m.test(text), `spurious top-level section: \n${text}`);
  assert.equal(text.match(/Characters$/gm).length, 1, "section duplicated");
}

// Matching is case-insensitive, mirroring the render's case-folded section name.
{
  const { text } = scaffoldCharacter("%% characters\n");
  assert.ok(text.includes("%%% New Character"), `case-insensitive match failed:\n${text}`);
}

console.log("character: all assertions passed");
