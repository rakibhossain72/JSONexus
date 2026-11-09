from setuptools import setup, find_packages

with open("README.md", "r") as fh:
    l_description = fh.read()


setup(
    name="jsonexus",
    version="1.1.0",
    packages=find_packages(),
    long_description=l_description,
    long_description_content_type="text/markdown",
    description="JSONexus is a lightweight and flexible database solution designed for modern applications. It provides a simple and intuitive way to store, query, and manipulate data using JSON documents.",
    author="Rakib Hossain",
    author_email="rakib4ggp@gmail.com",
    license="MIT",
    keywords="json-database",
    url="https://github.com/rakibma7254/jsonexus",
    python_requires=">=3.8",
    install_requires=["orjson"],
    classifiers=[
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.6",
    ],
)
