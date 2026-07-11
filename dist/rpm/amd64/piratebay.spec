Name:           piratebay
Version:        0.3.0
Release:        1%{?dist}
Summary:        Search and download torrents from The Pirate Bay from your terminal

License:        MIT

BuildArch:      x86_64

%description
Search and download torrents from The Pirate Bay right from your terminal.

%prep
# Prepare the build environment

%build
# Build steps (if any)

%install
mkdir -p %{buildroot}/usr/bin
cp -r %{_sourcedir}/amd64/usr %{buildroot}/

%files
/usr/bin/piratebay
