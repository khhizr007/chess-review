import html from "../../index.html?raw";
import { describe, expect, test } from "vitest";
import { FAQ } from "./faq";

interface LdQuestion {
  name: string;
  acceptedAnswer: { text: string };
}

describe("FAQ structured data", () => {
  test("index.html's FAQPage matches the FAQ shown on the page", () => {
    const json = /<script type="application\/ld\+json">([\s\S]*?)<\/script>/.exec(html)?.[1];
    expect(json).toBeDefined();
    const graph = (JSON.parse(json ?? "{}") as { "@graph": { "@type": string; mainEntity?: LdQuestion[] }[] })["@graph"];
    const page = graph.find((node) => node["@type"] === "FAQPage");
    expect(page?.mainEntity?.map((e) => ({ q: e.name, a: e.acceptedAnswer.text }))).toEqual(FAQ);
  });
});
