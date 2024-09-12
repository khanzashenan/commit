/**
 * Creating a sidebar enables you to:
 - create an ordered group of docs
 - render a sidebar for each doc of that group
 - provide next/previous navigation

 The sidebars can be generated from the filesystem, or explicitly defined here.

 Create as many sidebars as you want.
 */

// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  // By default, Docusaurus generates a sidebar from the docs folder structure
  // tutorialSidebar: [{ type: 'autogenerated', dirName: '.' }],


  tutorialSidebar: [
    'introduction',
    'overview',
    {
      type: 'category',
      label: 'Get started',
      link: {
        type: 'generated-index',
      },
      collapsed: false,
      items: [
        'get_started/overview',
        'get_started/configuration',
        {
          type: 'category',
          label: 'Running',
          link: {
            type: 'generated-index',
          },
          items: [
            'get_started/running/docker',
            'get_started/running/binary',

          ],
        },
        'get_started/troubleshooting',
      ],
    },
    {
      type: 'category',
      label: 'Developing',
      link: {
        type: 'generated-index',
      },
      items: [
        'developing/custom-modules',
        'developing/commit-module',
      ],
    },
    {
      type: 'category',
      label: 'Architecture',
      link: {
        type: 'generated-index',
      },
      items: [
        'architecture/overview',
      ],
    },
  ],

  // But you can create a sidebar manually
  /*
  tutorialSidebar: [
    'intro',
    'hello',
    {
      type: 'category',
      label: 'Tutorial',
      items: ['tutorial-basics/create-a-document'],
    },
  ],
   */
};

export default sidebars;
